import init, { RepoIndex } from "repoask-wasm";
import { fetchRepoFiles } from "./repo-fetcher.js";

const repoInput = document.getElementById("repo-input") as HTMLInputElement;
const loadBtn = document.getElementById("load-btn") as HTMLButtonElement;
const status = document.getElementById("status") as HTMLDivElement;
const searchBox = document.getElementById("search-box") as HTMLDivElement;
const queryInput = document.getElementById("query-input") as HTMLInputElement;
const searchBtn = document.getElementById("search-btn") as HTMLButtonElement;
const resultsDiv = document.getElementById("results") as HTMLDivElement;

let index: RepoIndex | null = null;

function setStatus(msg: string) {
  status.textContent = msg;
}

loadBtn.addEventListener("click", async () => {
  const spec = repoInput.value.trim();
  const match = spec.match(/^([^/]+)\/([^@]+)(?:@(.+))?$/);
  if (!match) {
    setStatus("Invalid format. Use owner/repo or owner/repo@ref");
    return;
  }
  const [, owner, repo, ref] = match;

  loadBtn.disabled = true;
  searchBox.classList.remove("visible");
  resultsDiv.innerHTML = "";

  try {
    setStatus("Initializing WASM...");
    await init();

    setStatus(`Fetching ${owner}/${repo}@${ref ?? "HEAD"}...`);

    const result = await fetchRepoFiles(owner, repo, ref ?? "HEAD", {
      extensions: ["ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs", "md", "mdx"],
      concurrency: 15,
      onProgress: (fetched, total) => {
        setStatus(`Fetching files: ${fetched}/${total}`);
      },
    });

    setStatus(`Building index from ${result.files.length} files (${(result.files.reduce((s, f) => s + f.content.length, 0) / 1024).toFixed(0)} KB)...`);

    index = new RepoIndex();
    for (const file of result.files) {
      index.addFile(file.path, file.content);
    }
    index.build();

    setStatus(`Ready. ${index.docCount()} documents indexed from ${result.files.length} files in ${(result.durationMs / 1000).toFixed(1)}s.`);
    searchBox.classList.add("visible");
    queryInput.focus();
  } catch (err) {
    setStatus(`Error: ${err}`);
  } finally {
    loadBtn.disabled = false;
  }
});

function doSearch() {
  if (!index) return;
  const query = queryInput.value.trim();
  if (!query) return;

  const t0 = performance.now();
  const json = index.search(query, 20);
  const elapsed = performance.now() - t0;
  const results: unknown[] = JSON.parse(json);

  resultsDiv.innerHTML = `<div class="stats">${results.length} results in ${elapsed.toFixed(1)}ms</div>`;

  for (const result of results) {
    const div = document.createElement("div");
    div.className = "result";

    if (isCode(result)) {
      const r = (result as { Code: CodeResult }).Code;
      div.innerHTML = `
        <span class="result-type code">code</span>
        <span class="result-name">${esc(r.name)}</span>
        <span class="result-score">${r.score.toFixed(2)}</span>
        <div class="result-path">${esc(r.filepath)} <span class="result-lines">:${r.start_line}-${r.end_line}</span> ${esc(r.kind)}</div>
      `;
    } else if (isDoc(result)) {
      const r = (result as { Doc: DocResult }).Doc;
      div.innerHTML = `
        <span class="result-type doc">doc</span>
        <span class="result-name">${esc(r.section)}</span>
        <span class="result-score">${r.score.toFixed(2)}</span>
        <div class="result-path">${esc(r.filepath)}</div>
        <div class="result-snippet">${esc(r.snippet.slice(0, 200))}</div>
      `;
    } else if (isExample(result)) {
      const r = (result as { Example: ExampleResult }).Example;
      div.innerHTML = `
        <span class="result-type example">example</span>
        <span class="result-name">${esc(r.name)}</span>
        <span class="result-score">${r.score.toFixed(2)}</span>
        <div class="result-path">${esc(r.filepath)} <span class="result-lines">:${r.start_line}-${r.end_line}</span> ${esc(r.kind)}</div>
      `;
    }

    resultsDiv.appendChild(div);
  }
}

searchBtn.addEventListener("click", doSearch);
queryInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") doSearch();
});

interface CodeResult { filepath: string; name: string; kind: string; start_line: number; end_line: number; score: number; }
interface DocResult { filepath: string; section: string; snippet: string; score: number; }
interface ExampleResult { filepath: string; name: string; kind: string; start_line: number; end_line: number; score: number; }

function isCode(r: unknown): boolean { return typeof r === "object" && r !== null && "Code" in r; }
function isDoc(r: unknown): boolean { return typeof r === "object" && r !== null && "Doc" in r; }
function isExample(r: unknown): boolean { return typeof r === "object" && r !== null && "Example" in r; }

function esc(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
