# repoask

Code understanding tool for any repository. Local-first, no external services, fast.

```sh
repoask search  vercel/next.js "middleware authentication"  # find code and docs
repoask explore supabase/auth-js "session management"       # understand how to use a library
repoask trace   ./my-app src/auth/session.ts#UserSession    # trace impact of changes
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
- `--dir` — restrict results to files under a directory prefix. Repeat or use commas
- `--ext` — restrict results to file extensions. Repeat or use commas
- `--type` — restrict results to `code` or `doc`
- `--verbose` — print parse diagnostics to stderr when rebuilding the index, or note cache reuse on cache hits

Output includes code symbols (functions, classes, types) and documentation sections, ranked by relevance.

```sh
$ repoask search colinhacks/zod "parse error" --format text
[code] src/ZodError.ts:15-120  Class ZodError  (score: 0.847)
[doc]  README.md#error-handling  "ZodError is a subclass of Error..."  (score: 0.723)
[code] src/types.ts:340-365  Function safeParse  (score: 0.651)
```

Filter to docs under `docs/`:

```sh
repoask search owner/repo "authentication" --dir docs --ext md --type doc
```

### explore

Understand how to use an external repository. Surfaces docs, public APIs, types, and examples — top-down.

```sh
$ repoask explore supabase/auth-js "authentication setup" --format text
[doc]     README.md#quick-start
          "Install @supabase/auth-js and call createClient()..."
[api]     src/GoTrueClient.ts  signInWithPassword(credentials)
          → returns AuthResponse { user, session }
[api]     src/GoTrueClient.ts  signUp(credentials)
          → returns AuthResponse { user, session }
[example] examples/nextjs/middleware.ts  createMiddleware()
          lines 8-30
[type]    src/lib/types.ts  AuthResponse
          { user: User | null, session: Session | null }
```

*Coming soon.*

### trace

Trace impact of changes in your own repository. Shows call graphs, type dependencies, and affected files — center-out.

```sh
$ repoask trace ./my-app src/auth/session.ts#UserSession --format text
target:   src/types/session.ts:15  type UserSession

produces: (2 files)
  src/auth/login.ts       createSession()
  src/auth/refresh.ts     refreshSession()

consumes: (12 files)
  src/api/users.ts        getUser()          score: 0.95
  src/api/profile.ts      updateProfile()    score: 0.87
  src/middleware/auth.ts   requireAuth()      score: 0.82
  ... and 9 more

impact: high (23 references across 14 files)
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
