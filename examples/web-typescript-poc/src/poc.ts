/**
 * PoC: bit clone a real GitHub repo into memory
 *
 * Key finding: bit's rawFetch uses MoonBit CPS (continuation-passing style).
 * The lib.js wrapper must pass _cont/_err_cont callbacks.
 * We wrap rawFetch in a Promise manually.
 *
 * Usage: npm run poc
 */

// Use the raw exports to bypass the lib.js wrapper (which may be out of sync)
import {
  createMemoryHost,
  init,
  fetch,
  checkout,
  createFetchTransport,
  type BitGitBackend,
} from "@mizchi/bit/lib";

const REPO_URL = "https://github.com/colinhacks/zod.git";
const REPO_ROOT = "/repo";
const REF = "main";

// Extensions that repoask-parser handles (oxc + markdown)
const REPOASK_EXTENSIONS = new Set([
  "ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs",
  "md", "mdx",
]);

async function main() {
  console.log(`[poc] Cloning ${REPO_URL} (ref: ${REF}) into memory...`);

  const backend = createMemoryHost();
  const transport = createFetchTransport(globalThis.fetch);

  // Step 1: init + fetch
  const t0 = performance.now();
  init(backend, REPO_ROOT, REF);

  console.log("[poc] Fetching...");
  const fetchResult = await fetch(backend, REPO_ROOT, REPO_URL, transport, {
    refspec: REF,
  });
  console.log(`[poc] Fetch result: ${JSON.stringify(fetchResult)}`);

  // Step 2: checkout
  console.log("[poc] Checking out...");
  const target = fetchResult.commitId ?? "HEAD";
  checkout(backend, REPO_ROOT, target);

  const t1 = performance.now();
  console.log(`[poc] Clone completed in ${(t1 - t0).toFixed(0)}ms`);

  // Step 3: Walk filesystem
  const files = walkDirectory(backend, REPO_ROOT, "");
  console.log(`[poc] Total files found: ${files.length}`);

  // Step 4: Classify
  const supported: string[] = [];
  const unsupported: string[] = [];
  for (const filepath of files) {
    const ext = filepath.split(".").pop() ?? "";
    if (REPOASK_EXTENSIONS.has(ext)) {
      supported.push(filepath);
    } else {
      unsupported.push(filepath);
    }
  }

  console.log(`[poc] Supported by repoask-parser: ${supported.length} files`);
  console.log(`[poc] Unsupported: ${unsupported.length} files`);

  // Step 5: Read supported files
  console.log("\n[poc] Sample supported files:");
  let totalBytes = 0;
  let fileCount = 0;
  for (const filepath of supported) {
    try {
      const content = readFileAsString(backend, `${REPO_ROOT}/${filepath}`);
      totalBytes += content.length;
      fileCount++;
      if (fileCount <= 10) {
        console.log(`  ${filepath} (${content.length} bytes)`);
      }
    } catch {
      // skip unreadable
    }
  }

  console.log(
    `\n[poc] Total: ${fileCount} files, ${(totalBytes / 1024).toFixed(0)} KB`
  );
  console.log("[poc] Integration pattern validated with real repo.");
}

function walkDirectory(
  backend: BitGitBackend,
  root: string,
  relativePath: string,
): string[] {
  const fullPath = relativePath ? `${root}/${relativePath}` : root;
  const entries: string[] = [];

  let children: string[];
  try {
    children = Array.from(backend.readdir(fullPath));
  } catch {
    return entries;
  }

  for (const name of children) {
    if (name === ".git") continue;

    const childRelative = relativePath ? `${relativePath}/${name}` : name;
    const childFull = `${root}/${childRelative}`;

    if (backend.isDir(childFull)) {
      entries.push(...walkDirectory(backend, root, childRelative));
    } else if (backend.isFile(childFull)) {
      entries.push(childRelative);
    }
  }

  return entries;
}

function readFileAsString(backend: BitGitBackend, path: string): string {
  const data = backend.readFile(path);
  if (typeof data === "string") return data;
  if (data instanceof Uint8Array) return new TextDecoder().decode(data);
  if (data instanceof ArrayBuffer) return new TextDecoder().decode(data);
  return new TextDecoder().decode(Uint8Array.from(data as ArrayLike<number>));
}

main().catch((err) => {
  console.error("[poc] Fatal error:", err);
  process.exit(1);
});
