# bit integration findings

## Summary

bit (`@mizchi/bit`) の JS library API は **sync 操作のみ** がライブラリビルド (`src/lib`) で動作する。
`fetch()` / `push()` 等の async 操作はライブラリビルドでは **動作しない**。

## Root cause

1. `js_fetch_remote` は MoonBit の `pub async fn` として定義されている
2. MoonBit の async → JS コンパイルは CPS (continuation-passing style) に変換する
3. CPS 内で `Promise.wait` が呼ばれると、MoonBit coroutine scheduler の `current_coroutine()` が必要
4. **ライブラリビルド (`moon build --target js src/lib`) には coroutine runtime (`spawn`, `reschedule`, `run_async_main`) が含まれない**
5. CLI ビルド (`moon build --target js src/cmd/bit`) にはこれらが含まれる
6. 結果: lib.raw.js から `rawFetch()` を呼ぶと `current_coroutine()` → `undefined` → panic

## What works

- `init`, `add`, `commit`, `status`, `checkout`, `branchList`, `log` 等の同期操作
- `createMemoryHost()` によるインメモリバックエンド
- `writeString`, `readFile`, `readdir`, `isDir`, `isFile` によるファイル操作

## What doesn't work

- `fetch()` — remote clone/fetch (async, coroutine runtime required)
- `push()` — remote push (same issue)
- `statusText()` — async variant of status
- `stashPush()` — async
- `diffWorktree()` / `diffWorktreeStat()` — async

## Options for repoask-web

### Option A: GitHub tarball API (recommended for now)
- Browser の `fetch()` で `https://api.github.com/repos/{owner}/{repo}/tarball/{ref}` を取得
- tar.gz を JS で展開 (pako + untar)
- 展開したファイルを repoask-wasm の `addFile()` に渡す
- CORS proxy が必要 (GitHub API は CORS ヘッダを返さない場合がある)
- bit 不要、依存が軽い

### Option B: bit CLI ビルドを使う
- `bit.js` (CLI ビルド) には async runtime が含まれる
- ただし `require("process")` 等の Node.js 依存がある
- ブラウザで動かすには polyfill or 修正が必要
- bit 側の修正 (lib ビルドに async runtime を含める) を待つ方が健全

### Option C: bit 側に修正を提案
- lib ビルドに `run_async_main` + coroutine runtime を含めるよう PR
- または lib export に `runAsync(callback)` ヘルパーを追加
- 最も正しいが、upstream 依存

## Recommendation

**短期**: Option A (tarball API) でブラウザ版を動かす。
**中期**: Option C で bit にコントリビュートし、lib ビルドで async が使えるようにする。
