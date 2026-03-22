# repoask — Search code and docs in any GitHub repo

## What this tool does

`repoask` searches code symbols and documentation across any public GitHub repository using BM25 full-text search. No API keys, no Docker, no LLM — just a single binary.

It indexes functions, classes, types, interfaces, and markdown docs, then lets you search across all of them with one natural language query.

## When to use

- Looking up how a library works (API, authentication, configuration)
- Finding function signatures, type definitions, or class structures
- Searching README, docs, examples, and source code in one query
- Investigating an unfamiliar repository's codebase

## Commands

### search

```
repoask search <owner/repo> "<query>" [--limit N] [--format json|text]
```

- Default output: JSON lines (one JSON object per line)
- Default limit: 10 results
- First run clones the repo (adds ~1-3s). Subsequent runs use cache (~6ms)
- Supports `owner/repo@branch` or `owner/repo@v2.0.0` for version pinning

### cleanup

```
repoask cleanup [owner/repo]
```

Remove cached clone and index. Omit the argument to clean all repos.

## Output format

Each JSON line is one of three result types:

**Code result** (function, class, type, etc.):
```json
{"Code":{"filepath":"src/auth.ts","name":"validateToken","kind":"Function","start_line":42,"end_line":67,"score":12.5}}
```

**Doc result** (markdown section):
```json
{"Doc":{"filepath":"docs/auth.md","section":"Authentication","snippet":"This section explains how to...","score":10.2}}
```

**Example result** (code in examples/ directory):
```json
{"Example":{"filepath":"examples/auth/login.ts","name":"handler","kind":"Function","start_line":8,"end_line":30,"score":9.1}}
```

### Result type discrimination

Match on the top-level key to determine the result type:
- `"Code"` → source code symbol
- `"Doc"` → documentation section
- `"Example"` → example code (from `examples/`, `sample/`, or `demo/` directories)

### Symbol kinds

`kind` is one of: `Function`, `Method`, `Class`, `Struct`, `Enum`, `Interface`, `Type`, `Trait`, `Const`

## Supported languages

Code symbol extraction:
- **TypeScript/JavaScript** (via oxc-parser): `.ts`, `.tsx`, `.js`, `.jsx`, `.mts`, `.cts`, `.mjs`, `.cjs`
- **Rust**: `.rs`
- **Python**: `.py`, `.pyi`
- **Go**: `.go`
- **Java**: `.java`
- **C/C++**: `.c`, `.h`, `.cpp`, `.cc`, `.hpp`
- **Ruby**: `.rb`

Documentation: all `.md` and `.mdx` files.

## Usage patterns

### Find how to use a library feature

```
repoask search supabase/auth-js "authentication setup"
```

Read the top Doc results for setup instructions, then use Code results to find the actual API.

### Find a specific function or type

```
repoask search colinhacks/zod "ZodError class"
```

Use the `start_line`/`end_line` from Code results to read the exact source range.

### Explore an unfamiliar repo

```
repoask search owner/repo "main entry point CLI"
```

Start with broad queries to find the entry point, then narrow down.

### Read a specific code range after search

After getting a Code result with `start_line` and `end_line`, read the source:

```
# Using the cached clone path
cat ~/.cache/repoask/repos/github.com/<owner>/<repo>/repo/<filepath> | sed -n '<start_line>,<end_line>p'
```

## Tips

- Use natural language: "authentication middleware", "error handling", "database connection"
- Include technical terms: "JWT", "OAuth", "WebSocket" — the tokenizer handles camelCase splitting
- Results are ranked by BM25 relevance. Symbol name matches rank highest, then doc headings, then body text
- The `score` field is relative within a single query — don't compare scores across queries
- Use `--limit 20` if the default 10 results don't surface what you need
