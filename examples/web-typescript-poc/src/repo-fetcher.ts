/**
 * Fetch a GitHub repository's files using Trees API + raw.githubusercontent.com.
 *
 * No CORS proxy needed. Works in both Node.js and browsers.
 *
 * Architecture note:
 * This module is intentionally standalone. In the future, its output
 * (Map<filepath, content>) can be piped into:
 * - repoask-wasm's addFile() for search indexing
 * - bit's writeString() for git history operations
 */

/** A fetched file with its path and content. */
export interface RepoFile {
  path: string;
  content: string;
}

/** Options for fetching a repository. */
export interface FetchRepoOptions {
  /** File extensions to include (e.g. ["ts", "md"]). If empty, fetch all text files. */
  extensions?: string[];
  /** Maximum concurrent fetches for raw content. Default: 10. */
  concurrency?: number;
  /** Callback for progress reporting. */
  onProgress?: (fetched: number, total: number) => void;
  /** Maximum file size in bytes to fetch. Default: 1MB. Skip larger files. */
  maxFileSize?: number;
}

/** Result of fetching a repository. */
export interface FetchRepoResult {
  files: RepoFile[];
  /** Files that were skipped (binary, too large, fetch error). */
  skipped: string[];
  /** The resolved commit SHA. */
  commitSha: string;
  /** Total time in milliseconds. */
  durationMs: number;
}

/** Error response from GitHub API. */
interface GitHubErrorResponse {
  message?: string;
}

/** Tree entry from GitHub Trees API. */
interface TreeEntry {
  path: string;
  mode: string;
  type: "blob" | "tree";
  sha: string;
  size?: number;
}

/** Response from GitHub Trees API. */
interface TreeResponse {
  sha: string;
  tree: TreeEntry[];
  truncated: boolean;
}

const DEFAULT_EXTENSIONS = new Set([
  "ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs",
  "rs", "py", "pyi", "go", "java", "c", "h", "cpp", "cc", "hpp",
  "rb",
  "md", "mdx",
]);

const DEFAULT_CONCURRENCY = 10;
const DEFAULT_MAX_FILE_SIZE = 1_000_000; // 1MB

/**
 * Fetch a public GitHub repository's source files.
 *
 * Step 1: Trees API to get file listing (1 API request)
 * Step 2: raw.githubusercontent.com to fetch each file (N requests, parallelized)
 */
export async function fetchRepoFiles(
  owner: string,
  repo: string,
  ref: string = "HEAD",
  options: FetchRepoOptions = {},
): Promise<FetchRepoResult> {
  const t0 = performance.now();
  const extensions = options.extensions
    ? new Set(options.extensions)
    : DEFAULT_EXTENSIONS;
  const concurrency = options.concurrency ?? DEFAULT_CONCURRENCY;
  const maxFileSize = options.maxFileSize ?? DEFAULT_MAX_FILE_SIZE;

  // Step 1: Resolve ref to commit SHA and get tree
  const resolvedRef = await resolveRef(owner, repo, ref);
  const tree = await fetchTree(owner, repo, resolvedRef);

  if (tree.truncated) {
    console.warn(`[repo-fetcher] Tree was truncated (>100k entries). Some files may be missing.`);
  }

  // Step 2: Filter to supported files
  const candidates = tree.tree.filter((entry) => {
    if (entry.type !== "blob") return false;
    const ext = entry.path.split(".").pop() ?? "";
    if (!extensions.has(ext)) return false;
    if (entry.size != null && entry.size > maxFileSize) return false;
    // Skip common non-source paths
    if (entry.path.includes("node_modules/")) return false;
    if (entry.path.includes("vendor/")) return false;
    if (entry.path.includes(".min.")) return false;
    return true;
  });

  // Step 3: Fetch file contents with concurrency control
  const files: RepoFile[] = [];
  const skipped: string[] = [];
  let fetched = 0;

  const fetchFile = async (entry: TreeEntry): Promise<void> => {
    try {
      const content = await fetchRawContent(owner, repo, resolvedRef, entry.path);
      files.push({ path: entry.path, content });
    } catch {
      skipped.push(entry.path);
    }
    fetched++;
    options.onProgress?.(fetched, candidates.length);
  };

  // Process in batches for concurrency control
  for (let i = 0; i < candidates.length; i += concurrency) {
    const batch = candidates.slice(i, i + concurrency);
    await Promise.all(batch.map(fetchFile));
  }

  return {
    files,
    skipped,
    commitSha: resolvedRef,
    durationMs: performance.now() - t0,
  };
}

/** Resolve a ref (branch/tag/HEAD) to a commit SHA. */
async function resolveRef(owner: string, repo: string, ref: string): Promise<string> {
  if (/^[0-9a-f]{40}$/i.test(ref)) return ref;

  // Try as branch first, then tag
  for (const prefix of ["heads", "tags"]) {
    const url = `https://api.github.com/repos/${owner}/${repo}/git/refs/${prefix}/${ref}`;
    const response = await fetch(url);
    if (response.ok) {
      const data = await response.json();
      // Tag might be annotated (object.type === "tag"), need to dereference
      if (data.object?.type === "tag") {
        const tagUrl = data.object.url;
        const tagData = await (await fetch(tagUrl)).json();
        return tagData.object?.sha ?? data.object.sha;
      }
      return data.object?.sha;
    }
  }

  // Fallback: try as commit SHA prefix or default branch
  const repoUrl = `https://api.github.com/repos/${owner}/${repo}`;
  const repoData = await (await fetch(repoUrl)).json();
  if (ref === "HEAD") {
    return resolveRef(owner, repo, repoData.default_branch);
  }

  throw new Error(`Could not resolve ref: ${ref}`);
}

/** Fetch the full recursive tree for a commit. */
async function fetchTree(owner: string, repo: string, commitSha: string): Promise<TreeResponse> {
  // Get the commit to find the tree SHA
  const commitUrl = `https://api.github.com/repos/${owner}/${repo}/git/commits/${commitSha}`;
  const commitResponse = await fetch(commitUrl);
  if (!commitResponse.ok) {
    const error: GitHubErrorResponse = await commitResponse.json();
    throw new Error(`Failed to fetch commit: ${error.message ?? commitResponse.status}`);
  }
  const commitData = await commitResponse.json();
  const treeSha = commitData.tree.sha;

  // Get the recursive tree
  const treeUrl = `https://api.github.com/repos/${owner}/${repo}/git/trees/${treeSha}?recursive=1`;
  const treeResponse = await fetch(treeUrl);
  if (!treeResponse.ok) {
    const error: GitHubErrorResponse = await treeResponse.json();
    throw new Error(`Failed to fetch tree: ${error.message ?? treeResponse.status}`);
  }

  return treeResponse.json();
}

/** Fetch a single file's raw content from raw.githubusercontent.com. */
async function fetchRawContent(
  owner: string,
  repo: string,
  ref: string,
  path: string,
): Promise<string> {
  const url = `https://raw.githubusercontent.com/${owner}/${repo}/${ref}/${path}`;
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to fetch ${path}: ${response.status}`);
  }
  return response.text();
}
