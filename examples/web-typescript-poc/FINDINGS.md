# bit integration findings

## Summary

bit (`@mizchi/bit`) の JS library API は **sync 操作のみ** がライブラリビルド (`src/lib`) で動作する。
`fetch()` / `push()` 等の async 操作はライブラリビルドでは **動作しない**。

これは意図的な設計ではなく、lib ビルドの async サポートが未完成な状態。

## Root cause

1. `js_fetch_remote` は MoonBit の `pub async fn` として定義 (`src/lib/js_api_exports.mbt:1248`)
2. MoonBit async → JS は CPS (continuation-passing style) に変換される
3. CPS 内で `@js_async.Promise.wait` が呼ばれると、`@coroutine.current_coroutine()` が必要
4. coroutine runtime (`spawn`, `reschedule`) は `moonbitlang/async` パッケージに含まれる
5. **lib の `moon.pkg` は `moonbitlang/async` をテスト用にしか import していない**
6. CLI の `src/cmd/bit` は `moonbitlang/async` を import しており、`run_async_main` が含まれる
7. 結果: lib.raw.js に `spawn`/`reschedule` がなく、`current_coroutine()` → `undefined` → panic

### 具体的なコード箇所

- `src/lib/moon.pkg`: `moonbitlang/async/js_async` は通常 import、`moonbitlang/async` は `for "test"` のみ
- `moonbitlang/async/src/integration.mbt`: JS target の `run_async_main` は `@coroutine.spawn(main)` + `@event_loop.reschedule()`
- `npm/lib.js:989`: `rawFetch(...)` を普通に呼んでいるが、CPS の `_cont`/`_err_cont` 引数を渡していない

### 意図ではない根拠

- `js_fetch_remote` が lib の JS export に含まれている (`moon.pkg` の `exports` リスト)
- npm/README.md に `fetch()` の使用例が記載されている
- npm/lib.d.ts に `fetch()` の型定義がある
- browser demo (`docs/demo/`) は fetch/push を使っていない (sync 操作のみ)
- lib.js wrapper が `rawFetch()` を普通に `await` している (Promise 化の仕組みがない)

## What works

- `init`, `add`, `commit`, `status`, `checkout`, `branchList`, `log` 等の同期操作
- `createMemoryHost()` によるインメモリバックエンド
- `writeString`, `readFile`, `readdir`, `isDir`, `isFile` によるファイル操作

## What doesn't work

- `fetch()` — remote clone/fetch (async, coroutine runtime required)
- `push()` — remote push (same issue)
- `statusText()` — async
- `stashPush()` — async
- `diffWorktree()` / `diffWorktreeStat()` — async

## Fix (upstream)

lib の `moon.pkg` で `moonbitlang/async` を通常 import に昇格すれば、
JS lib ビルドに coroutine runtime が含まれ、async 操作が動くようになるはず。

```diff
 import {
   ...
   "moonbitlang/async/js_async" @js_async,
+  "moonbitlang/async" @async,
   ...
 }
-
-import {
-  "moonbitlang/async" @async,
-  ...
-} for "test"
```

ただし tree-shaking でサイズ増、副作用の有無は要検証。

## Options for repoask-web

### Option A: GitHub tarball API (short-term)
- `https://api.github.com/repos/{owner}/{repo}/tarball/{ref}` で tar.gz 取得
- JS で展開 → repoask-wasm の `addFile()` に渡す
- CORS proxy 必要
- bit 不要

### Option B: bit upstream fix (mid-term)
- `moon.pkg` の import 修正を PR
- lib ビルドで async が動くようになれば、bit の fetch() でブラウザ内 git clone が可能
- 最も正しい解決策

### Option C: async runtime 手動注入 (workaround)
- CLI ビルド (bit.js) から `spawn`/`reschedule`/`run_async_main` を抽出
- lib.raw.js にパッチ適用
- 脆い、バージョンアップで壊れる
