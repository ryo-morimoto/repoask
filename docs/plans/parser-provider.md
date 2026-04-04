# Parser Provider: 動的パーサー選択アーキテクチャ

## 概要

どの platform (CLI native / Web WASM) でも、各言語ごとに最適の AST パーサーを動的に選択して実行できるようにする。
使い始めの体験（初回ロードサイズ）と使ってる間の体験（parse 品質・対応言語の広さ）を両立する。

## 現状の問題

- repoask-wasm は repoask-parser (oxc + pulldown-cmark) のみ依存。TS/JS と Markdown しか parse できない
- repoask-treesitter (7言語) を WASM に含めると grammar 1つ 100-300KB でサイズ爆発
- 現在の WASM: 631KB (gzip 236KB)。全 grammar 追加で +700KB〜2MB
- parse.rs が repoask-parser → repoask-treesitter を直接呼んでおり、platform 間で共有不能

## Ideal

- コア WASM (~250KB gzip) にはパーサー dispatch 層 + BM25 + tokenizer のみ。言語固有コードゼロ
- パーサーは「拡張子 → パーサー artifact」のレジストリで解決。CLI はローカルから、Web は必要時に CDN から fetch
- 各言語に最適なパーサーを選べる (TS/JS は oxc、他は tree-sitter、将来は専用パーサーも差し替え可能)
- `addFile()` の呼び出し側はパーサーの存在を意識しない。拡張子を渡せば裏で適切なパーサーがロード・実行される
- パーサーのロードは 1 回だけ。同じ言語の 2 ファイル目以降はキャッシュヒット

## Known edge cases（今は解かない）

- パーサー artifact のバージョン不整合（コアとパーサーの API contract mismatch）
- CDN ダウン / オフライン環境での Web 利用
- tree-sitter grammar の WASM ビルドサイズ最適化（grammar ごとの wasm-opt）
- 複数言語が同じ拡張子を持つ場合（`.h` → C or C++）
- パーサーロード中のプログレス表示
- CLI でもプラグイン分離するか、feature flag で静的リンクのままにするか

## Steps

1. **Now**: `ParseProvider` trait を導入。実装は現状のまま静的リンク。WASM には oxc + markdown だけ含む
2. **When tree-sitter を Web に載せたくなったとき**: Web 側に動的ロード機構を追加。tree-sitter grammar を個別 `.wasm` として配布
3. **When 新しい言語パーサーを追加するとき**: レジストリを拡張。CLI は feature flag、Web は CDN から取得
4. → Ideal: platform 問わず拡張子だけで最適パーサーが動的選択・実行される

## Step 1 設計

### ParseProvider trait

```rust
// repoask-core/src/parse.rs (新規)

pub trait ParseProvider {
    /// ファイルを解析して IndexDocument のリストを返す。
    /// 拡張子未対応なら Ok(None)、パース失敗なら Err。
    fn parse_file(
        &self,
        filepath: &str,
        source: &str,
    ) -> Result<Option<Vec<IndexDocument>>, ParseError>;
}
```

### Platform 別実装

**CLI (repoask-repo)**:

```rust
pub struct NativeParseProvider;

impl ParseProvider for NativeParseProvider {
    fn parse_file(&self, filepath: &str, source: &str) -> Result<Option<Vec<IndexDocument>>, ParseError> {
        // 1. repoask-parser (oxc + markdown) を試行
        // 2. Unsupported → repoask-treesitter にフォールバック
        // 3. それも Unsupported → Ok(None)
    }
}
```

**Web (repoask-wasm)**:

```rust
pub struct WasmParseProvider;

impl ParseProvider for WasmParseProvider {
    fn parse_file(&self, filepath: &str, source: &str) -> Result<Option<Vec<IndexDocument>>, ParseError> {
        // repoask-parser のみ（oxc + markdown）
        // 未対応拡張子は Ok(None)
    }
}
```

### 変更箇所

| 箇所 | Before | After |
|---|---|---|
| repoask-core | パーサー無関係 | `ParseProvider` trait を定義 |
| repoask-repo/parse.rs | 直接呼び出し | `NativeParseProvider` impl |
| repoask-wasm/lib.rs | `parse_file_lenient` 直接呼び出し | `WasmParseProvider` 経由 |
| バイナリサイズ | 変化なし | 変化なし |

### Step 2 への拡張点

`WasmParseProvider` に `register_parser(ext, wasm_bytes)` を追加すれば、JS 側から動的に grammar を登録できる:

```js
// 将来像（Step 1 では作らない）
const grammar = await fetch("/grammars/tree-sitter-python.wasm");
index.registerGrammar("py", await grammar.arrayBuffer());
```
