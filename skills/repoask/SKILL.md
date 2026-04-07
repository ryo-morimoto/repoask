---
name: repoask
description: Search code and docs in any GitHub repository using BM25 full-text search. Use when looking up library APIs, finding function signatures, or investigating unfamiliar codebases.
---

# repoask

Search code symbols and documentation across any public GitHub repository. No API keys, no Docker, no LLM.

## When to use

When you need to search code or documentation in an external GitHub repository.

## Prerequisites

```
repoask --version   # check installation
cargo install repoask  # install if missing
```

## Commands

```
repoask search <owner/repo[@ref]> "<query>" [--limit N] [--format json|text]
repoask cleanup [owner/repo]
```

- First run clones the repo (~1-3s). Subsequent runs use cache (~6ms)
- Default: 10 results, JSON lines output
- **Queries must be in English.** Non-ASCII alphabetic characters are rejected
- Exit code 1 + stderr message on error

## Output format

Each line is an independent JSON object. Match on the top-level key:

| Key | Type | Fields |
|---|---|---|
| `"Code"` | Source code symbol | `filepath`, `name`, `kind`, `start_line`, `end_line`, `score` |
| `"Doc"` | Markdown section | `filepath`, `section`, `snippet`, `score` |

`kind`: `Function` \| `Method` \| `Class` \| `Struct` \| `Enum` \| `Interface` \| `Type` \| `Trait` \| `Const`

Code in `examples/`, `sample/`, or `demo/` directories has `is_example: true`.

Example:

```json
{"Code":{"filepath":"src/auth.ts","name":"validateToken","kind":"Function","start_line":42,"end_line":67,"score":12.5,"is_example":false}}
{"Doc":{"filepath":"docs/auth.md","section":"Authentication","snippet":"This section explains how to...","score":10.2}}
```

The `score` is relative within a single query — do not compare across queries.

## Example

```
repoask search supabase/auth-js "authentication setup"
```

Use `start_line`/`end_line` from results to read the exact source range.
