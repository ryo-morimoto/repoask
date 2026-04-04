# repoask 設計思想

## これは何か

任意のリポジトリに対するコード理解ツール。

外部repoの使い方を調べるときも、自分のrepoをリファクタするときも、同じツールで完結する。

## 核心の痛み

**コードを理解するまでの待ち時間が長い。**

Context7はクラウド依存で対応ライブラリが限られる。既存のコード検索ツールは遅いか依存が重い。repoaskは「任意のrepo、ローカル完結、速い」で解決する。

## 誰が使うか

- 開発者が外部repoの使い方を調べるとき
- 開発者が自分のrepoをリファクタ・レビューするとき
- coding agentが外部repoを参照するとき

## 3つの入口

```
repoask search  owner/repo "query"    # 汎用検索（どのrepoでも）
repoask explore owner/repo "query"    # 使い方を知る（外部repo向け）
repoask trace   owner/repo file/sym   # 影響範囲を追う（自分のrepo向け）
```

### search — 汎用キーワード検索

BM25 + ASTシンボル検索。コードとドキュメントを横断してヒットする。どのrepoに対しても使える基本コマンド。

### explore — 外部repoの仕様理解

「このライブラリの認証ってどうやるの？」に答える。docs → 公開API → 型 → 実装例 → 内部実装の順に**上から下へ**潜る。Context7のコード特化版。

### trace — 自分のrepoの影響範囲追跡

「この関数を変えたら何が壊れる？」に答える。変更点 → 依存先 → 依存元 → 影響範囲の順に**中心から外へ**広がる。コールグラフ + 型依存グラフが基盤。

## 2つの価値層

| 層 | 問い | 機能 | データ |
|---|---|---|---|
| **検索** | 「これ何？どう使う？」 | BM25 + ASTシンボル検索 | shallow clone |
| **理解** | 「変えたらどうなる？」 | コールグラフ + 型依存グラフ | full clone |

## 設計原則

### 1. コマンドがデータ要件を決める

ルーティングロジック不要。各コマンドが必要とするデータレベルは静的に決まる。

- `search` / `explore` → shallow clone（`--depth 1`）で十分
- `trace` → full cloneが必要。なければ自動でfetchする

### 2. 足りなければ勝手に取る

ユーザーはデータレベルを意識しない。コマンドを叩いたら、足りないデータがあれば裏で取得される。キャッシュがあればスキップ。

```
repoask search vercel/next.js "middleware"   # 初回: shallow clone（数秒）
repoask search vercel/next.js "routing"      # キャッシュヒット（0.1秒）
repoask trace vercel/next.js src/server/...  # full cloneに自動昇格（追加数秒、以降キャッシュ）
```

### 3. データが増えると結果がリッチになる

同じインターフェース、同じクエリでも、キャッシュにあるデータが増えた分だけ出力が豊かになる。full clone済みなら関連ファイルのパスと重みが追加される。重みが高ければプレビューも展開される。

---

# 型設計

Option地獄を避け、enumバリアントごとに必要なフィールドだけ持つdiscriminated union設計。

## SearchResult（current）

```rust
pub enum SearchResult {
    Code(CodeResult),
    Doc(DocResult),
}

pub struct CodeResult {
    pub filepath: String,
    pub name: String,
    pub kind: SymbolKind,
    pub start_line: u32,
    pub end_line: u32,
    pub score: f32,
    pub is_example: bool,
}

pub struct DocResult {
    pub filepath: String,
    pub section: String,
    pub snippet: String,
    pub score: f32,
}
```

Planned: full-clone enrichments such as `related` / `preview` are not implemented yet.

## IndexDocument（パーサー出力 → インデックス入力の中間表現）

```rust
pub enum IndexDocument {
    Code(Symbol),
    Doc(DocSection),
}
```

## Symbol（repoask-parser）

```rust
pub enum SymbolKind { Function, Method, Class, Struct, Enum, Interface, Type, Trait, Const }

pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub filepath: String,
    pub start_line: u32,
    pub end_line: u32,
    pub doc_comment: Option<String>,
    pub params: Vec<String>,
}
```

## DocSection（repoask-parser）

```rust
pub struct DocSection {
    pub filepath: String,
    pub section_title: String,
    pub heading_hierarchy: Vec<String>,
    pub content: String,
    pub code_symbols: Vec<String>,  // fenced block内のidentifier
    pub start_line: u32,
    pub end_line: u32,
}
```

## TraceResult（planned; trace コマンドは未実装）

```rust
pub struct TraceResult {
    pub target: SymbolRef,
    pub references: Vec<Reference>,
    pub impact_score: ImpactScore,
}

pub struct SymbolRef {
    pub filepath: String,
    pub name: String,
    pub kind: SymbolKind,
    pub start_line: u32,
    pub end_line: u32,
}

pub struct Reference {
    pub filepath: String,
    pub name: String,
    pub kind: ReferenceKind,
    pub start_line: u32,
    pub end_line: u32,
}

pub enum ReferenceKind {
    Calls,      // この関数を呼んでいる
    CalledBy,   // この関数から呼ばれている
    UsesType,   // この型を使っている
    UsedByType, // この型に使われている
}

pub enum ImpactScore {
    Low,    // 参照少、末端に近い
    Medium, // 中程度の参照
    High,   // 参照多、コアモジュール
}
```

## CodeGraph（planned internal representation）

```rust
pub struct CodeGraph {
    pub nodes: Vec<SymbolRef>,
    pub edges: Vec<Edge>,
}

pub struct Edge {
    pub from: usize, // nodes index
    pub to: usize,   // nodes index
    pub kind: ReferenceKind,
}
```

## BM25フィールド重み

| FieldId | 対象 | 重み | ソース |
|---|---|---|---|
| 0 | シンボル名 / 見出しテキスト | 4.0 | Symbol.name / DocSection.section_title |
| 1 | docstring / コメント / 本文 | 2.0 | Symbol.doc_comment / DocSection.content |
| 2 | 引数名 / コードブロック内シンボル | 1.5 | Symbol.params / DocSection.code_symbols |
| 3 | ファイルパス | 1.0 | トークン化した相対パス |
