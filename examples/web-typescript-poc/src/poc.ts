/**
 * PoC: Trees API + raw fetch → simulated repoask-wasm search
 *
 * Validates the full browser-compatible pipeline:
 * 1. Fetch file tree from GitHub (Trees API, CORS OK)
 * 2. Fetch file contents (raw.githubusercontent.com, CORS OK)
 * 3. Feed files to search index (simulated, will be repoask-wasm)
 *
 * Usage: npm run poc
 */

import { fetchRepoFiles } from "./repo-fetcher.js";

const OWNER = "colinhacks";
const REPO = "zod";
const REF = "main";

async function main() {
  console.log(`[poc] Fetching ${OWNER}/${REPO}@${REF}...`);

  const result = await fetchRepoFiles(OWNER, REPO, REF, {
    extensions: ["ts", "tsx", "js", "md", "mdx"],
    concurrency: 15,
    onProgress: (fetched, total) => {
      if (fetched % 50 === 0 || fetched === total) {
        console.log(`[poc] Progress: ${fetched}/${total} files`);
      }
    },
  });

  console.log(`\n[poc] Fetch completed in ${result.durationMs.toFixed(0)}ms`);
  console.log(`[poc] Commit: ${result.commitSha}`);
  console.log(`[poc] Files fetched: ${result.files.length}`);
  console.log(`[poc] Files skipped: ${result.skipped.length}`);

  // Summarize by extension
  const byExt = new Map<string, number>();
  let totalBytes = 0;
  for (const file of result.files) {
    const ext = file.path.split(".").pop() ?? "?";
    byExt.set(ext, (byExt.get(ext) ?? 0) + 1);
    totalBytes += file.content.length;
  }

  console.log(`[poc] Total size: ${(totalBytes / 1024).toFixed(0)} KB`);
  console.log("[poc] By extension:");
  for (const [ext, count] of [...byExt.entries()].sort((a, b) => b[1] - a[1])) {
    console.log(`  .${ext}: ${count} files`);
  }

  // Show sample files
  console.log("\n[poc] Sample files:");
  for (const file of result.files.slice(0, 10)) {
    console.log(`  ${file.path} (${file.content.length} bytes)`);
  }

  // Simulate what repoask-wasm would do
  console.log("\n[poc] Simulating repoask-wasm addFile() + build() + search()...");
  const t0 = performance.now();

  // In the real version:
  // const index = new RepoIndex();
  // for (const file of result.files) {
  //   index.addFile(file.path, file.content);
  // }
  // index.build();
  // const results = index.search("parse error validation", 10);

  const t1 = performance.now();
  console.log(`[poc] Simulated index build: ${(t1 - t0).toFixed(0)}ms`);

  if (result.skipped.length > 0) {
    console.log(`\n[poc] Skipped files (first 10):`);
    for (const path of result.skipped.slice(0, 10)) {
      console.log(`  ${path}`);
    }
  }

  console.log("\n[poc] Pipeline validated. Ready for repoask-wasm integration.");
}

main().catch((err) => {
  console.error("[poc] Fatal error:", err);
  process.exit(1);
});
