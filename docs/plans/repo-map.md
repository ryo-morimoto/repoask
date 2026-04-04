# Repo Map: agent-first `overview` / `search` / symbol inspect 設計

## 概要

repoask を coding agent が直接使う主な場面は、「特定の機能にバグがある」「想定と違う挙動が出ている」「使っている API の実態を追いたい」ときである。
この用途では README 要約よりも、**公開 API / 公開型 / 公開 API のテスト** が重要な入口になる。

repoask の制約は変わらない。

- LLM なし
- embedding なし
- SQLite などの別ストレージなし
- CLI と WASM で同じ Rust core を使う
- CLI surface をこれ以上増やさない

そのため repoask の次の設計は、README/tree-first な repo map ではなく、
**public surface first の investigation surface** として組み直す。

## 目的

- `overview` で公開 API / 型 / 公開 API テストの入口を返す
- `search` で feature/concept 起点の候補を、次の作業に使いやすい card 形式で返す
- symbol 指定の inspect で、内部実装 tree / raw code / 呼び先 card / relevant tests / next hints を返す
- すべて deterministic かつ explainable に生成する
- 情報量は token budget を意識して制御する

## 解かないこと

- LLM で自然文 summary を作ること
- docs.rs や GitHub API を叩いて外部から説明文を取ること
- `extract`, `explore`, `trace` をそれぞれ独立コマンドとして増やし続けること
- token 上限を無視して大量の raw context を丸投げすること

## アプローチ比較

### A. README/tree-first overview

- どう動くか: README, docs, directory tree, 主要ファイルを返す
- 強み: 人間の onboarding には分かりやすい
- 弱み: bug investigation の入口として弱い。agent が次に呼ぶ API / 見る test を決めにくい
- 向いている場面: repo の一般紹介

### B. Public surface first investigation surface

- どう動くか: overview で public API / type / test を返し、search と inspect で内部実装に降りる
- 強み: coding agent の調査フローに近い。README に依存しない。LLM なしでも成立する
- 弱み: export 判定、test linkage、comment 正規化などの設計が要る
- 向いている場面: 今回の対象

### C. Graph + git signal + context personalization debugger

- どう動くか: call graph / type graph / git history / 会話文脈を統合して再ランクする
- 強み: 最終形として強い
- 弱み: 先に graph と signal 基盤が要る
- 向いている場面: Step 2 以降

**採用方針**: Step 1 は B。構造は C に伸びるように作る。

## Ideal

- `overview` が README 要約ではなく public API / public types / public tests を返す
- `search` が単なる BM25 hit list ではなく、「次に何を読むか」が分かる card を返す
- symbol inspect が raw code だけでなく、implementation tree / callees / tests / hints を返す
- comment 由来情報は JSDoc / rustdoc / docstring を構造化して使う
- renderer が token budget を意識して sections ごとに出し分ける

## Known edge cases（今は解かない）

- public/private/exported の定義が言語ごとに揃っていない
- test file が `tests/`, `__tests__/`, `spec/` などの慣習に従っていない
- 巨大関数の raw code を全文返すと budget を超える
- re-export や facade module 越しの API surface 判定が難しい
- parser coverage が薄い言語では call tree や type linkage が欠ける

## Steps

1. **Now**: `overview` を public surface first に作り直す。公開 API / 型 / 公開 API テストを deterministic に返す
2. **When search output is upgraded**: `search` を card 化し、publicness / test proximity / comment quality を加味した ranking にする
3. **When inspect lands**: symbol 指定で implementation tree / raw code / callee cards / relevant tests / next hints を返す
4. **When BoostSignals / graph land**: search と inspect の ranking / tree expansion に graph boost を合成する
5. **When parser coverage grows**: export surface / return type / type refs / doc comment normalization を言語横断で揃える

→ Ideal: coding agent が API から内部実装に降りる調査を、少ない tool call で end-to-end に進められること。

## CLI surface の原則

- 行動として必要なのは **overview / search / inspect** の 3 つだけ
- `extract` は独立コマンドにしない。inspect が raw code 取得を内包する
- `trace` は長期的には inspect / graph expansion に吸収する
- exact syntax は実装時に詰めるが、**top-level verb を増やすより surface を畳む** 方向を優先する

例:

```sh
repoask overview colinhacks/zod@v4
repoask search colinhacks/zod@v4 "object"
repoask colinhacks/zod@v4 zod:object   # target syntax は仮。意味は symbol inspect
```

## Step 1: `overview` 設計

### 返すべき情報

- public APIs
- public types
- public API tests
- 次に inspect/search へ進むための entry hints

README 要約と directory tree は主役ではない。必要なら fallback か hints の補助情報にとどめる。

### Text output 例

```text
repo: colinhacks/zod@v4

public_api:
  z.object(shape, params?) -> ZodObject
  z.strictObject(shape) -> ZodObject
  z.looseObject(shape) -> ZodObject

public_types:
  ZodObject<Shape, UnknownKeys, Catchall, Output, Input>
  ZodRawShape
  ZodTypeAny

public_api_tests:
  packages/zod/src/__tests__/object.test.ts  object_parses_valid_shape
  packages/zod/src/__tests__/object.test.ts  object_rejects_unknown_keys

entry_hints:
  inspect api:z.object
  inspect type:ZodObject
  read test:object.test.ts
```

### JSON shape

```rust
pub struct InvestigationOverview {
    pub public_apis: Vec<PublicApiCard>,
    pub public_types: Vec<PublicTypeCard>,
    pub public_api_tests: Vec<TestCard>,
    pub entry_hints: Vec<HintCard>,
}

pub struct PublicApiCard {
    pub symbol_ref: String,
    pub filepath: String,
    pub signature: String,
    pub comment_summary: Option<String>,
    pub tests: Vec<TestRef>,
    pub score: f32,
}

pub struct PublicTypeCard {
    pub symbol_ref: String,
    pub filepath: String,
    pub kind: SymbolKind,
    pub signature: String,
    pub comment_summary: Option<String>,
    pub score: f32,
}

pub struct TestCard {
    pub filepath: String,
    pub test_name: String,
    pub linked_symbols: Vec<String>,
    pub score: f32,
}

pub struct HintCard {
    pub label: String,
    pub target: String,
}
```

## `search` 設計

### 役割

`search` は concept/feature 起点の候補探索であり、結果は次の tool call を減らす shape で返す。
単なる `SearchResult` の配列ではなく、**why surfaced / next actions / test linkage** を含む card に寄せる。

### Step 1 ranking

Step 1 の ranking は graph 非依存の deterministic heuristic に留める。

```text
investigation_score = bm25
                    + public_api_bonus
                    + public_type_bonus
                    + linked_test_bonus
                    + comment_summary_bonus
                    + exact_symbol_bonus
                    + future_graph_boost
```

signal 例:

- exported public API: `+3.0`
- exported public type: `+2.0`
- linked test がある: `+2.0`
- `comment_summary` がある: `+0.3`
- example/demo path: `x0.7`

graph boost は Step 2 以降に `BoostSignals` / call graph / type graph から差し込む。

### Output 例

```text
1. api:z.object
   file: packages/zod/src/index.ts
   signature: object(shape, params?) -> ZodObject
   why:
   - exact symbol token match
   - exported public API
   - linked tests: 12
   next:
   - inspect api:z.object
   - inspect type:ZodObject
   - read test:object.test.ts
```

## symbol inspect 設計

### 役割

inspect は public API / type / symbol を起点に、内部実装を追うための調査出力を返す。
`extract` と `trace` を別々に増やす代わりに、inspect が raw code と graph-like な次導線をまとめて返す。

### Output shape

```rust
pub struct SymbolInspect {
    pub target: TargetCard,
    pub implementation_tree: Vec<TreeNode>,
    pub raw_code: RawCodeBlock,
    pub callees: Vec<CalleeCard>,
    pub relevant_tests: Vec<TestCard>,
    pub hints: Vec<HintCard>,
    pub truncated: Vec<TruncationNote>,
}

pub struct TargetCard {
    pub symbol_ref: String,
    pub filepath: String,
    pub signature: String,
    pub comment_summary: Option<String>,
}

pub struct TreeNode {
    pub symbol_ref: String,
    pub relation: TreeRelation,
    pub depth: u8,
}

pub struct RawCodeBlock {
    pub filepath: String,
    pub start_line: u32,
    pub end_line: u32,
    pub code: String,
    pub is_truncated: bool,
}

pub struct CalleeCard {
    pub symbol_ref: String,
    pub filepath: String,
    pub signature: String,
    pub comment_summary: Option<String>,
    pub linked_tests: Vec<TestRef>,
    pub next_hints: Vec<String>,
}
```

### implementation tree の作り方

- Step 1 は target symbol の **direct callees / imported symbols / referenced types** に限定する
- tree depth は 1-2 を基本にする
- graph store がない段階では、target body の AST から symbol references を抽出して local tree を組む
- Step 2 以降で call graph / type graph に置き換える

### 呼び先 summary の作り方（LLM 不使用）

自然文 summary を生成しない。
代わりに **structured callee card** を作り、その card から短い deterministic line を render する。

source priority:

1. `CommentInfo.summary_line`
2. `signature_preview`
3. `kind + params + linked_types + linked_tests` の構造化情報

例:

```text
ZodObject.create(shape, params) -> ZodObject
doc: Creates a new object schema from a raw shape.
tests: object.test.ts, pickomit.test.ts
next: inspect processUnknownKeys, inspect type:ZodRawShape
```

`doc:` がない場合は prose を捏造せず、signature/test/type 情報だけを出す。

## JSDoc / rustdoc / docstring 活用設計

### 原則

- comment は全文 string のまま使わず、**構造化して保持** する
- docs.rs は別データソースではなく、Rust source の `///` / `//!` を一次ソースとして扱う
- network fetch はしない

### 追加する正規化型

```rust
pub struct CommentInfo {
    pub summary_line: Option<String>,
    pub body_preview: Option<String>,
    pub tags: Vec<CommentTag>,
    pub examples: Vec<String>,
    pub source: CommentSource,
}

pub enum CommentSource {
    JsDoc,
    RustDoc,
    PythonDocstring,
    PlainComment,
}
```

### JSDoc

- 既存の oxc comment 抽出を元に `/** ... */` を正規化する
- `@param`, `@returns`, `@throws`, `@deprecated`, `@example` を tag として保持する
- 先頭 paragraph を `summary_line` にする

### Rust / docs.rs

- `///` と `//!` を rustdoc source として扱う
- 先頭 paragraph を `summary_line` にする
- `# Examples`, `# Errors`, `# Panics` を section tag として抽出する
- docs.rs 相当の情報は source comment から再構成し、外部 fetch はしない

### Python docstring

- まだ未実装だが、TODO にある通り tree-sitter で `expression_statement > string` を拾う
- Step 2 で `PythonDocstring` として `CommentInfo` に揃える

## Public API / type / test linkage 設計

### Public API / public type

parser output に少なくとも以下の metadata を足す。

```rust
pub struct ExportInfo {
    pub is_public: bool,
    pub export_kind: ExportKind,
    pub container: Option<String>,
}
```

- TS/JS: `export`, `export default`, re-export を判定
- Rust: `pub`, `pub(crate)`, crate root re-export を区別
- 他言語は parser coverage に応じて段階導入

### Test linkage

test は deterministic heuristic で結びつける。

signal:

- test file path (`tests/`, `__tests__/`, `spec/`, `test/`)
- test name / snapshot name に symbol 名が出る
- test body で symbol が呼ばれる
- import path が public API file と近い

`overview` と `inspect` は top-N の linked tests を返す。

## Token budget / renderer 設計

### 背景

情報が少なすぎると tool call が増える。
情報が多すぎると、agent が切り捨て判断をする推論コストが増える。

そのため renderer は「全部返す」のではなく、**section ごとの budget を持つ**。

### Budget の考え方

- token の厳密計測はせず、line/char/item を proxy にする
- mode ごとに `RenderBudget` を持つ
- raw code を最優先しつつ、tests / callees / hints を必要最低限に絞る

例:

```rust
pub struct RenderBudget {
    pub max_items_per_section: usize,
    pub max_tree_depth: u8,
    pub max_code_lines: usize,
    pub max_comment_chars: usize,
    pub max_total_chars: usize,
}
```

初期値の目安:

- overview: APIs 6, types 6, tests 6, hints 4
- search: result cards 8
- inspect: tree depth 2, callees 5, tests 5, hints 5, raw code 120 lines

### Truncation order

低優先から削る。

1. extra hints
2. extra tests
3. low-score callees
4. tree の深い枝
5. raw code の末尾

削ったときは `truncated` と `next_hints` を必ず返す。

## 変更箇所

| 箇所 | 変更 |
|---|---|
| `repoask-core::types` | `CommentInfo`, `ExportInfo`, `InvestigationOverview`, `SymbolInspect`, `RenderBudget` などを追加 |
| `repoask-parser` / `repoask-treesitter` | JSDoc / rustdoc / docstring を `CommentInfo` に正規化。export/public metadata を追加 |
| `repoask-core::index` | public API / type / test linkage に使う metadata を保持し、overview/search/inspect 用の集約 API を持たせる |
| `repoask-repo` | `overview(...)`, card-aware `search(...)`, `inspect(...)` を追加 |
| `cli` | `overview` を実装し、独立 `extract` ではなく inspect behavior を追加。exact syntax は CLI surface 最小化を優先して決める |
| `wasm` | investigation outputs を返す API を後追いで追加 |

## 追加しないもの

- LLM summary cache
- docs.rs / GitHub への追加 fetch
- 新しい永続 DB
- freeform prose summary

## テスト

- JSDoc 正規化 snapshot test
- Rustdoc 正規化 snapshot test
- public API / public type 判定 test
- public API と test linkage の ranking test
- `overview` snapshot test
- `search` card ranking test
- `inspect` snapshot test（tree / raw code / callees / tests / hints）
- budget truncation test

## 最小の成功条件

1. `overview` を見れば README なしで public API / type / test の入口が分かる
2. `search` を見れば次の tool call が 1 手で決まる
3. inspect を見れば raw code を読みつつ、次に追う callee と relevant tests が分かる

> `repoask` が coding agent にとって「一般的な repo 要約ツール」ではなく、「壊れた機能を API 起点で追う investigation tool」になること。
