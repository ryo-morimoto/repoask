# repoask

## Development Rules

### Dogfooding: Use repoask to search dependencies

When investigating external repositories (dependencies, reference implementations, etc.), use `repoask` CLI instead of manually cloning or browsing GitHub.

```bash
# Example: search oxc-parser's API
repoask search oxc-project/oxc "parse typescript"

# Example: search tree-sitter's query API
repoask search tree-sitter/tree-sitter "QueryCursor matches"

# Example: search bit's JS library API
repoask search bit-vcs/bit "createMemoryHost fetch transport"
```

This serves two purposes:
1. Dogfooding — we find usability issues and search quality problems firsthand
2. Efficiency — faster than cloning and grepping manually

---

# repoask 仕様書

## What

`owner/repo` を渡すだけで、任意のGitHubリポジトリのコードとドキュメントに対して自然言語で検索できるツール。

Context7の体験をローカルで再現する。「このライブラリの認証ってどうやるの？」と聞いたら、README・docs・examples・実装コードを横断して答えが返ってくる。

CLIでもブラウザでも動く。外部サービス依存ゼロ。パフォーマンスで既存ツールを圧倒する。

## Why

coding agentが外部リポジトリを参照するとき、現状2つの不満がある:

**Context7的なもの（ドキュメント検索）はクラウド依存で、対応ライブラリが限られる。**
Context7は便利だが、インデックスされていないライブラリには使えない。
任意の `owner/repo` に対してすぐ使えるローカル版が必要。

**既存のコード検索ツールは遅いか、依存が重い。**
code-graph-rag → Docker + Memgraph + LLM API が必要、検索にLLM呼び出しで数秒。
probe → インデックスなし毎回フルAST parse、検索に2-3秒。
GitNexus → Node.js + KuzuDB、インデックス構築に数十秒。
Greptile → SaaS、初回インデックスに数時間。

repoaskは「インデックス構築3秒、検索100ms、外部依存ゼロ」で差別化する。

## Core Concept

**agentは `owner/repo` と自然言語のクエリだけ渡す。それ以外は全部ツールがやる。**

```
repoask search vercel/next.js "middleware authentication"
→ [
    {type: "doc",  file: "docs/.../13-middleware.md", section: "Middleware", snippet: "Middleware allows you to run code before a request..."},
    {type: "code", file: "packages/next/src/server/web/adapter.ts", name: "adapter", line: 23-67, kind: "function"},
    {type: "example", file: "examples/with-iron-session/pages/api/login.ts", name: "handler", line: 8-30, kind: "function"},
  ]
```

ドキュメント・コード・exampleが1つのクエリで横断的にヒットする。

## 配布形態と優先度

| 形態 | 優先度 | 説明 |
|---|---|---|
| **CLI** | **一級** | `repoask search owner/repo "query"` |
| **Agent Skills** | **一級** | SKILL.md / AGENTS.md に書くだけでagentが使える |
| **Web** | **一級** | ブラウザで完結。同一コアロジックのWASMビルド |
| **ライブラリAPI** | 二級 | Rust crateとして他ツールに組み込み |
| **MCP Server** | 三級 | 必要になったら対応。初期は不要 |

## 必須要件 (MUST)

### M1: owner/repo 指定のみで動作する

- `owner/repo` を渡すだけ。cloneもインデックスも裏で全部やる
- 2回目以降はキャッシュヒットでclone不要
- `owner/repo@branch` や `owner/repo@v2.0.0` でバージョン指定も可能

### M2: コードとドキュメントを横断して自然言語検索できる

検索対象は2種類:

**コードシンボル（AST由来）:**
- 関数・クラス・メソッド・構造体・型定義
- シンボル名をcamelCase/snake_case分割して自然言語トークン化
  - `validateJWTToken` → `"validate" "jwt" "token"`
- 引数名、コメント、docstring もインデックスに含む
- 検索結果は行範囲付き（agentがその範囲だけ読める）

**ドキュメント（テキスト由来）:**
- README.md, docs/, examples/, CHANGELOG.md, *.md 全般
- セクション単位でチャンキング（見出しで分割）
- コード例もインデックスに含む（fenced code block内のシンボル名）

両者を統合した1つのインデックスに対して、1つのクエリで検索できる。
結果には `type: "code" | "doc" | "example"` が付く。

### M3: embeddingもLLMも外部サービスも使わない

- すべてローカルで完結する
- BM25全文検索 + AST構造解析だけで検索品質を出す
- ネットワーク接続は初回cloneのみ
- Docker不要、データベースサーバー不要、APIキー不要

### M4: 大規模リポジトリでもインデックス構築は3秒以内

- git clone時間は除く（ネットワーク依存で制御不能）
- clone後のAST解析 + ドキュメント解析 + インデックス構築が3秒以内
- 2回目以降はインデックスがキャッシュから読まれるため0秒

### M5: 検索レイテンシは100ms以内

- インデックス構築済みの状態で、1クエリの応答が100ms以内
- 結果のフォーマット:
  - コード: `{type, filepath, name, kind, start_line, end_line, score}`
  - ドキュメント: `{type, filepath, section, snippet, score}`

### M6: ASTレベルの構造理解

- テキストの行マッチではなく、関数・クラス・構造体などの単位で返す
- 「この関数は何行目から何行目」がわかる
- agentはこの行範囲で `sed -n '23,67p' file` して必要な部分だけ読む

### M7: 環境を汚さない

- cloneは `/tmp` 配下（またはユーザー指定のキャッシュディレクトリ）
- ユーザーのワーキングディレクトリには何も書き込まない
- `repoask cleanup` で全キャッシュ一括削除
- `repoask cleanup owner/repo` で特定repoだけ削除

### M8: CLI + Agent Skills が一級市民

- CLIのstdoutはagentが直接パースできるフォーマット（JSON lines or 構造化テキスト）
- agent skillとして `SKILL.md` を同梱する。agentはこれを読むだけで使い方がわかる
- 特別なサーバープロセスやデーモンは不要。1コマンド実行して結果が返る

### M9: CLIとWebで同一コアが動く

- コアロジック（AST抽出、トークナイザ、転置インデックス、BM25）はRustで実装
- CLIはnativeバイナリ、Webは同一コードのWASMビルド
- Webではブラウザ内で完結する。サーバーサイド不要

## 技術選定

### 言語: Rust

コアロジックをすべてRustで実装する。CLIはnativeバイナリ、Webはwasm32ターゲット。

選定理由:
- tree-sitterの本家がRust。native bindingで最速のAST parse
- TS/JSにはoxc-parser（Rust native）が使える。mizchi/similarityが実証済み
- BM25 + 転置インデックスを自前実装しても100行程度。外部DB依存を完全排除
- 同一コードからnativeとWASMの両ビルドが出せる
- `cargo install` + napi-rsでnpmバイナリ配布も可能

### AST parse: tree-sitter (+ oxc-parser for TS/JS)

| 環境 | tree-sitter | oxc-parser |
|---|---|---|
| CLI (native) | Rust crate直接リンク、最速 | Rust crate直接リンク、最速 |
| Web (WASM) | web-tree-sitter（公式WASM版） | oxc WASMビルド or web-tree-sitter fallback |

parse結果は共通の中間表現（シンボルリスト）に変換してからインデックスに渡す。

### BM25検索: 自前実装（Rust）

外部DB（DuckDB, SQLite, tantivy）に依存しない。理由:

- BM25のコアは数十行の数学。自前で書いてもバグのリスクが低い
- 転置インデックスは `HashMap<String, Vec<(DocId, TermFreq)>>` で十分
- 数万シンボル規模なら素朴な実装で100ms以内
- WASMビルドでもそのまま動く。DB bindingのWASM対応を心配する必要がない
- フィールド別重み付け（シンボル名 > docstring > ファイルパス）を自由に制御できる

インデックスの永続化は `bincode` でバイト列にシリアライズ → ファイル保存。

### Git操作

| 環境 | 手段 |
|---|---|
| CLI | `git clone --depth 1` (subprocess) |
| Web | GitHub tarball API (`/repos/{owner}/{repo}/tarball/{ref}`) or bit (MoonBit WASM git) |

WebでのGit cloneはCORS制約があるため、GitHub APIのtarball取得が現実的。
bit (mizchi/bit-vcs) はMoonBit製のWASM git実装で、将来的に統合を検討。

### ファイルウォーク

| 環境 | 手段 |
|---|---|
| CLI | `ignore` crate（ripgrepと同じ.gitignore完全対応） |
| Web | tarball展開後のin-memoryファイルツリー走査 |

### 識別子トークナイザ

Rust純粋関数。camelCase/snake_case/PascalCase分割 + 英語stemming (porter)。
CLIでもWASMでも同一コード。

### npm配布

napi-rsでRustバイナリをnpmパッケージとして配布。
`npm install -g repoask` でインストール、probeやrepomixと同じ方式。

## 推奨要件 (SHOULD)

### S1: コードブロック抽出

- 検索結果のシンボルの実コードを取得する `extract` サブコマンド
- `repoask extract owner/repo src/auth/jwt.ts:42` → 関数全体を返す
- `repoask extract owner/repo src/auth/jwt.ts#validateToken` → シンボル名で指定

### S2: ディレクトリ / 拡張子フィルタ

- `--dir src --ext ts,js` で検索範囲を絞れる
- `--type code` でコードのみ、`--type doc` でドキュメントのみ

### S3: BM25ランキングのフィールド重み付け

- シンボル名 > docstring/コメント > 引数名 > ファイルパス の順で重み
- ドキュメントの見出しは本文より重み高

### S4: boolean検索構文

- `"jwt OR token AND (verify OR validate)"` のような構文
- agentがクエリ精度を上げたいときに使う

### S5: 複数リポジトリ横断検索

- `repoask search owner/repo1 owner/repo2 "query"` で複数repoまとめて検索

### S6: repo概要の取得

- `repoask overview owner/repo` でREADME要約 + ディレクトリ構造 + 主要エクスポートの一覧を返す
- agentが「まずこのrepoが何なのか」を掴むためのエントリポイント

## 想定ワークフロー

### Context7的な使い方: ライブラリの使い方を調べる

```
Human: "Supabaseの認証をNext.jsで使うにはどうすればいい？"

Agent:
  1. repoask search supabase/auth-js "authentication setup nextjs"
     → [doc] README.md#quick-start: "Install @supabase/auth-js..."
     → [doc] docs/guides/nextjs.md#middleware: "Create middleware for session refresh..."
     → [example] examples/nextjs/middleware.ts: createMiddleware()
     → [code] src/GoTrueClient.ts: signInWithPassword(credentials)

  2. 上位のdoc結果でセットアップ手順を把握
  3. 必要に応じて repoask extract で signInWithPassword の実装を読む
  4. ユーザーに手順 + コード例を提示
```

### コード実装の深掘り

```
Human: "Zodのparse関数ってエラーをどう構造化してる？"

Agent:
  1. repoask search colinhacks/zod "parse error structure ZodError"
     → [code] src/ZodError.ts: class ZodError (line 15-120)
     → [code] src/types.ts: safeParse() (line 340-365)
     → [doc] README.md#error-handling: "ZodError is a subclass of Error..."

  2. repoask extract colinhacks/zod src/ZodError.ts:15
     → ZodError クラス全体のコードを取得
  3. 構造を説明
```

### 未知のリポジトリの初期調査

```
Human: "このライブラリ何？ mizchi/similarity"

Agent:
  1. repoask overview mizchi/similarity
     → README要約 + ディレクトリ構造 + "Rust製コード類似度検出ツール。oxc-parserとtree-sitterでAST解析..."

  2. repoask search mizchi/similarity "main entry point CLI"
     → [code] crates/similarity-ts/src/main.rs: fn main()
     → [doc] README.md#quick-start
  3. ユーザーに概要を提示
```

## 既存ツールとの差別化

| | repoask | code-graph-rag | probe | Context7 | GitNexus |
|---|---|---|---|---|---|
| セットアップ | **0依存** | Docker+Memgraph+LLM | cargo install | クラウド | npx |
| インデックス構築 | **<3s** | 数分 | なし(毎回parse) | サーバー側 | 数十秒 |
| 検索速度 | **<100ms** | 数秒(LLM) | 2-3s | ネットワーク依存 | 数百ms |
| Web動作 | **✓ WASM** | ✗ | ✗ | ✓(SaaS) | ✓(WASM) |
| owner/repo直指定 | **✓** | ✗ | ✗ | ✓ | ✓(Web) |
| ドキュメント検索 | **✓** | ✗ | ✗ | ✓ | ✗ |
| LLM不要 | **✓** | ✗ | ✓ | ✗ | ✗ |
| グラフ走査 | ✗ | ✓ | ✗ | ✗ | ✓ |

repoaskの立ち位置: **速度とゼロ依存に全振りした、ローカルファーストのコード+ドキュメント検索。**
グラフ走査（CALLS関係、blast radius分析）はスコープ外。そこが必要ならGitNexusやcode-graph-ragを使う。

## 非スコープ

- コード変更・PR作成（読み取り専用ツール）
- プライベートリポジトリ対応（初期バージョンでは公開repoのみ）
- リアルタイムのファイルウォッチ（静的なスナップショットインデックス）
- embedding / LLM呼び出しによるセマンティック検索
- ナレッジグラフ / コールグラフ走査
- MCP Server（必要になったら対応）
