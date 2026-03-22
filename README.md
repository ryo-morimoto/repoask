# repoask

Search code and docs in any GitHub repository. Local-first, zero dependencies, fast.

```sh
repoask search vercel/next.js "middleware authentication"
```

## Install

```sh
cargo install repoask
```

## Commands

### search

Search a repository's code and documentation using natural language.

```sh
repoask search owner/repo "query"
```

Pin to a specific ref:

```sh
repoask search owner/repo@v2.0.0 "query"
```

Options:

- `-n`, `--limit` — max results (default: 10)
- `-f`, `--format` — output format: `json` (default) or `text`

Output includes code symbols (functions, classes, types) and documentation sections, ranked by relevance.

```sh
$ repoask search colinhacks/zod "parse error" --format text
[code] src/ZodError.ts:15-120  Class ZodError  (score: 0.847)
[doc]  README.md#error-handling  "ZodError is a subclass of Error..."  (score: 0.723)
[code] src/types.ts:340-365  Function safeParse  (score: 0.651)
```

### explore

Understand how to use an external repository. Surfaces docs, public APIs, types, and examples — top-down.

```sh
repoask explore supabase/auth-js "authentication setup"
```

*Coming soon.*

### trace

Trace impact of changes in your own repository. Shows call graphs, type dependencies, and affected files — center-out.

```sh
repoask trace my-repo src/auth/session.ts#UserSession
```

*Coming soon.*

### cleanup

Remove cached data.

```sh
repoask cleanup              # remove all
repoask cleanup owner/repo   # remove specific repo
```

## How It Works

1. Shallow clones the repo on first use (`git clone --depth 1`)
2. Parses code with [oxc](https://github.com/oxc-project/oxc) (TS/JS) and [tree-sitter](https://tree-sitter.github.io/) (Rust, Python, Go, Java, C, C++, Ruby)
3. Parses documentation (Markdown) into sections
4. Builds a BM25 inverted index over symbols, docs, and file paths
5. Caches everything locally — subsequent searches return in <100ms

No LLM, no embedding, no external service, no Docker.

## Supported Languages

| Language | Parser |
|---|---|
| TypeScript, JavaScript | oxc |
| Rust, Python, Go, Java, C, C++, Ruby | tree-sitter |
| Markdown | built-in |

## License

MIT
