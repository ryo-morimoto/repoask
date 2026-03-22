# repoask 型設計

Option地獄を避け、enumバリアントごとに必要なフィールドだけ持つdiscriminated union設計。

## SearchResult

```rust
pub enum SearchResult {
    Code(CodeResult),
    Doc(DocResult),
    Example(ExampleResult),
}

pub struct CodeResult {
    pub filepath: String,
    pub name: String,
    pub kind: SymbolKind,
    pub start_line: u32,
    pub end_line: u32,
    pub score: f32,
}

pub struct DocResult {
    pub filepath: String,
    pub section: String,
    pub snippet: String,
    pub score: f32,
}

pub struct ExampleResult {
    pub filepath: String,
    pub name: String,
    pub kind: SymbolKind,
    pub start_line: u32,
    pub end_line: u32,
    pub score: f32,
}
```

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

## BM25フィールド重み

| FieldId | 対象 | 重み | ソース |
|---|---|---|---|
| 0 | シンボル名 / 見出しテキスト | 4.0 | Symbol.name / DocSection.section_title |
| 1 | docstring / コメント / 本文 | 2.0 | Symbol.doc_comment / DocSection.content |
| 2 | 引数名 / コードブロック内シンボル | 1.5 | Symbol.params / DocSection.code_symbols |
| 3 | ファイルパス | 1.0 | トークン化した相対パス |
