# repoask ハーネス設計

## 設計思想

**「趣味をコンパイルエラーにする」** — LLMが書きがちなダメコードを、ツールチェーンが自動的に弾く。人間のレビューに頼らない。

## レイヤー構成

```
┌─────────────────────────────────────────────────┐
│ Layer 0: 型システム（コンパイル時）              │
│   crate境界 / newtype / discriminated union      │
├─────────────────────────────────────────────────┤
│ Layer 1: Lint（コンパイル時）                    │
│   clippy pedantic + nursery + restriction deny   │
├─────────────────────────────────────────────────┤
│ Layer 2: フォーマット + スペルチェック（pre-commit）│
│   rustfmt + typos + prek                         │
├─────────────────────────────────────────────────┤
│ Layer 3: テスト（pre-push + CI）                │
│   nextest + insta snapshot + proptest            │
├─────────────────────────────────────────────────┤
│ Layer 4: 品質ゲート（CI）                       │
│   coverage ratchet + mutation testing + cargo-deny│
├─────────────────────────────────────────────────┤
│ Layer 5: リリース（CI on main）                 │
│   release-plz + cross-rs + divan benchmark       │
└─────────────────────────────────────────────────┘
```

各レイヤーは下位レイヤーの保証の上に成り立つ。Layer 0 が最も安価（コンパイル時ゼロコスト）で最も強力。上に行くほどコストが上がるが、下位で漏れたものを捕捉する。

---

## Layer 0: 型システム

### crate境界で依存方向を強制

```
repoask-core     → 外部依存ゼロ（serde のみ）。Domain型の定義
repoask-parser   → core に依存。AST/Markdown → Symbol/DocSection
repoask-repo     → core に依存。git clone + キャッシュ管理
cli/             → 全crateに依存。CLIアダプタ（anyhow許可）
```

**コンパイラが建築テストになる:** `repoask-core` は物理的に `repoask-cli` を import できない。Cargo.toml に依存がなければコンパイルエラー。テスト不要。

### Discriminated Union（Option地獄の排除）

```rust
// BAD: どのフィールドがどの状態で有効か型で表現できない
pub struct SearchResult {
    pub kind: SearchResultKind,
    pub symbol_name: Option<String>,
    pub section: Option<String>,
    // ...
}

// GOOD: バリアントごとに必要なフィールドだけ持つ
pub enum SearchResult {
    Code(CodeResult),
    Doc(DocResult),
    Example(ExampleResult),
}
```

詳細は [DESIGN.md](DESIGN.md) 参照。

### Newtype Pattern

```rust
// BAD: 任意の文字列がクエリになる
fn search(index: &Index, query: &str) -> Vec<SearchResult> { ... }

// GOOD: 構築時にバリデーション、以降は信頼
pub struct Query(String);

impl Query {
    pub fn new(raw: &str) -> Result<Self, QueryError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(QueryError::Empty);
        }
        Ok(Self(trimmed.to_owned()))
    }
}

fn search(index: &Index, query: &Query) -> Vec<SearchResult> { ... }
```

原則: **"Parse, Don't Validate"** — オブジェクトが存在する時点で不変条件が成立。

### エラーハンドリング規約

| crate | パターン | 理由 |
|---|---|---|
| `repoask-core` | `thiserror` 型付きenum | 呼び出し元がmatchできる |
| `repoask-parser` | `thiserror` 型付きenum | parse失敗 vs 未対応言語 vs IOを区別 |
| `repoask-repo` | `thiserror` 型付きenum | clone失敗 vs キャッシュ破損 vs ロック競合を区別 |
| `cli/` | `anyhow` | トップレベルバイナリ、人間向けエラー表示 |

各ライブラリcrateに `pub type Result<T> = std::result::Result<T, XxxError>;` を定義。`Box<dyn Error>` 禁止。

```rust
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum IndexError {
    #[error("unsupported language: {0}")]
    UnsupportedLanguage(String),
    #[error("parse failed for {path}: {reason}")]
    ParseFailed { path: PathBuf, reason: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

`#[non_exhaustive]` でmatch armにワイルドカードを強制。LLMが全バリアントを知っている前提のコードを防ぐ。

---

## Layer 1: Lint

### 設定ファイル

- `Cargo.toml` → `[workspace.lints.rust]` + `[workspace.lints.clippy]`
- `clippy.toml` → 複雑度・行数・引数数の上限
- 各crate `Cargo.toml` → `[lints] workspace = true`

### LLM対策で特に効くlint

| LLMのミス | 弾くlint | レベル |
|---|---|---|
| `.unwrap()` 乱用 | `unwrap_used` | deny |
| `.expect()` でお茶を濁す | `expect_used` | deny |
| `todo!()` / `unimplemented!()` 放置 | `todo`, `unimplemented` | deny |
| `println!` デバッグ残し | `print_stdout`, `print_stderr` | deny |
| `#[allow(...)]` で黙らせる | `allow_attributes_without_reason` | deny |
| `Result`返す関数内でpanic | `unwrap_in_result` | deny |
| `panic!` マクロ | `panic` | deny |
| 不必要な `.clone()` | `clone_on_ref_ptr` | warn |
| 無関係な変数シャドウイング | `shadow_unrelated` | warn |
| `unsafe` コード | `unsafe_code` (rustc lint) | forbid |

### clippy.toml の閾値

```toml
cognitive-complexity-threshold = 15    # 認知的複雑度
excessive-nesting-threshold = 5        # ネスト深度
too-many-lines-threshold = 100         # 関数の行数
too-many-arguments-threshold = 6       # 引数の数
min-ident-chars-threshold = 2          # 識別子の最小文字数
```

---

## Layer 2: Pre-commit（prek）

### 設定ファイル: `prek.toml`

git hookの管理に [prek](https://github.com/j178/prek)（Rust-native pre-commit）を使用。

**pre-commit（毎コミット、瞬時に終わるもののみ）:**
- `cargo fmt -- --check`
- `typos`（スペルチェック）
- `trailing-whitespace`, `check-toml`, `end-of-file-fixer`

**pre-push（push前、重いが共有前に必ず通す）:**
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo nextest run --workspace`
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`

### セットアップ

```bash
cargo install prek
prek install
```

---

## Layer 3: テスト

### テストランナー: cargo-nextest

設定ファイル: `.config/nextest.toml`

| プロファイル | fail-fast | retries | 用途 |
|---|---|---|---|
| `default` | true | 0 | ローカル開発（即座にフィードバック） |
| `ci` | false | 2 | CI（全テスト実行、flaky対策） |

### テスト種類と使い分け

| 種類 | ツール | 対象 | 実行タイミング |
|---|---|---|---|
| Unit | nextest | 純粋関数（tokenizer, BM25計算） | pre-push + CI |
| Integration | nextest | parse → index → search のE2E | pre-push + CI |
| Snapshot | insta | 検索結果の構造、シリアライズ形式 | pre-push + CI |
| Property | proptest | tokenizer roundtrip、BM25スコア順序不変条件 | CI |
| Mutation | cargo-mutants `--in-diff` | PR差分のみ | CI (PR) |
| Benchmark | divan | tokenizer throughput、検索レイテンシ | CI (main) |
| Doc tests | `cargo test --doc` | ドキュメント内コード例 | CI |

### Snapshot テスト（insta）

LLMが検索ロジックを変えたとき、snapshot差分がPRに出る。assertion-basedだと pass/fail しか見えないが、snapshotは**実際の出力**がレビューできる。

```rust
#[test]
fn test_search_results_format() {
    let results = index.search(&query, 5);
    insta::assert_json_snapshot!(results);
}
```

CI では `CI=1 cargo test` でスナップショットの自動更新を禁止。差分があればテスト失敗。

### Property-Based テスト（proptest）

repoaskで特に有効なケース:
- **Tokenizer roundtrip:** `tokenize(join(tokens))` が元のトークンのスーパーセットを返す
- **BM25スコア順序:** ドキュメントにクエリ語を追加するとスコアが上がる
- **Index serialize/deserialize:** `deserialize(serialize(index)) == index`

### テストフィクスチャ

```
tests/fixtures/
  sample_repo/
    src/
      auth.ts          # TS/JS パース検証用
      main.rs          # Rust パース検証用
      utils.py         # Python パース検証用
    docs/
      README.md        # Markdownチャンキング検証用
      guide.md         # 見出し階層テスト用
    examples/
      basic.ts         # Example検出テスト用
```

`include_str!` でコンパイル時に埋め込み、ファイルシステム依存を排除。

---

## Layer 4: 品質ゲート

### Coverage Ratchet

カバレッジは単調増加のみ許可。下がったらCIがfail。

```bash
CURRENT=$(cargo llvm-cov --json | jq '.data[0].totals.lines.percent')
BASELINE=$(cat .metrics/coverage-baseline)
# CURRENT >= BASELINE でなければ fail
```

| フェーズ | 閾値 | 理由 |
|---|---|---|
| 開発中 (0.1.x) | ベースラインなし | 有意義なテストを書くことに集中 |
| 安定化 (0.5.x) | 70% line coverage | core/parser のコアロジックをカバー |
| リリース (1.0) | 80% line coverage | 公開APIは全カバー |

ツール: [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov)（LLVM instrumentation、Linux/macOS/Windows対応）

### Mutation Testing

```bash
cargo mutants --in-diff origin/main...HEAD --test-tool nextest
```

PR差分のみを対象にすることで、CIの実行時間を現実的に保つ。surviving mutant（テストで殺せない変異）があればPRブロック。

### 依存監査（cargo-deny）

設定ファイル: `deny.toml`

- **脆弱性:** deny（既知の脆弱性があればfail）
- **ライセンス:** MIT/Apache-2.0/BSD系のみ許可、GPL deny
- **重複依存:** warn
- **openssl ban:** rustls を使う
- **ソース制限:** crates.io のみ、unknown git/registry は deny

---

## Layer 5: リリース

### release-plz

Conventional Commitsからバージョンを自動決定し、CHANGELOG.mdを生成。

1. mainにpushされるたびにrelease PRを自動更新
2. release PRをマージするとcrates.io公開 + GitHub Release作成 + タグ付け

### ベンチマーク回帰検出（divan）

```rust
#[divan::bench(args = [100, 1000, 10_000])]
fn search_index(bencher: divan::Bencher, n: usize) {
    let index = build_test_index(n);
    bencher.bench(|| index.search("authentication middleware"));
}
```

mainブランチのみでベンチマーク実行。仕様のパフォーマンス要件（インデックス構築 <3秒、検索 <100ms）を定量的に検証。

### クロスコンパイル（cross-rs）

タグ作成時に自動ビルド:
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`
- `wasm32-unknown-unknown`

---

## タスクランナー: just

設定ファイル: `justfile`

`just --list` でagentが利用可能なタスクを自己発見できる。

| レシピ | 実行内容 |
|---|---|
| `just fmt` | `cargo fmt --all` |
| `just lint` | `cargo clippy --workspace --all-targets -- -D warnings` |
| `just test` | `cargo nextest run --workspace` |
| `just snapshot` | `cargo insta test --workspace` |
| `just coverage` | `cargo llvm-cov nextest` → lcov出力 |
| `just doc` | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` |
| `just ci` | fmt + lint + test + doc（ローカルCI一発） |
| `just bench` | `cargo bench --workspace` |
| `just deny` | `cargo deny check` |
| `just clean` | `cargo clean` + 生成物削除 |

---

## 実行タイミングまとめ

| チェック | pre-commit | pre-push | CI (PR) | CI (main) | CI (weekly) |
|---|---|---|---|---|---|
| cargo fmt --check | x | | x | | |
| typos | x | | x | | |
| cargo clippy | | x | x | | |
| cargo nextest | | x | x | | |
| cargo doc (warnings) | | x | x | | |
| insta snapshot | | x | x | | |
| cargo-deny | | | x | | |
| cargo llvm-cov | | | | x | |
| cargo mutants --in-diff | | | x | | |
| divan benchmark | | | | x | |
| cargo mutants (全体) | | | | | x |
| release-plz | | | | x | |

---

## 設定ファイル一覧

| ファイル | 役割 | 状態 |
|---|---|---|
| `Cargo.toml` `[workspace.lints]` | clippy/rust lint設定 | 済 |
| `clippy.toml` | 複雑度・行数・引数数の上限 | 済 |
| `rustfmt.toml` | フォーマットルール | 済 |
| `deny.toml` | 依存監査・ライセンス・ソース制限 | 済 |
| `.config/nextest.toml` | テストランナープロファイル | 済 |
| `justfile` | タスクランナー | 済 |
| `prek.toml` | git hooks (pre-commit / pre-push) | 済 |

---

## 参考資料

### Lint / Clippy

- [Clippy Lint Groups and Documentation](https://doc.rust-lang.org/stable/clippy/lints.html)
- [Clippy Configuration Reference](https://doc.rust-lang.org/clippy/configuration.html)
- [Full Clippy Lint List (searchable)](https://rust-lang.github.io/rust-clippy/master/index.html)
- [rust-magic-linter — Strict Clippy for AI-assisted Rust](https://github.com/vicnaum/rust-magic-linter)
- [Rust Workspace Lints RFC 3389](https://rust-lang.github.io/rfcs/3389-manifest-lint.html)

### フォーマット

- [Rustfmt Configuration Reference](https://github.com/rust-lang/rustfmt/blob/main/Configurations.md)
- [Rustfmt Style Edition (2024)](https://doc.rust-lang.org/edition-guide/rust-2024/rustfmt-style-edition.html)

### 依存監査

- [cargo-deny GitHub](https://github.com/EmbarkStudios/cargo-deny)
- [cargo-deny Template Configuration](https://github.com/EmbarkStudios/cargo-deny/blob/main/deny.template.toml)
- [cargo-deny Check Configuration](https://embarkstudios.github.io/cargo-deny/checks/cfg.html)

### テスト

- [cargo-nextest Documentation](https://nexte.st/)
- [cargo-nextest Configuration Reference](https://nexte.st/docs/configuration/reference/)
- [insta Snapshot Testing](https://insta.rs/docs/quickstart/)
- [proptest vs quickcheck](https://altsysrq.github.io/proptest-book/proptest/vs-quickcheck.html)
- [Property-based testing in Rust with proptest](https://blog.logrocket.com/property-based-testing-in-rust-with-proptest/)
- [cargo-mutants Documentation](https://mutants.rs/)
- [cargo-mutants CI Integration](https://mutants.rs/ci.html)

### カバレッジ

- [cargo-llvm-cov GitHub](https://github.com/taiki-e/cargo-llvm-cov)
- [Coverage — Rust Project Primer](https://rustprojectprimer.com/measure/coverage.html)

### ベンチマーク

- [Divan benchmarking library](https://nikolaivazquez.com/blog/divan/)
- [CodSpeed divan support](https://codspeed.io/changelog/2025-02-13-divan-support)
- [Criterion.rs](https://github.com/bheisler/criterion.rs)

### Git Hooks

- [prek — Rust-native pre-commit](https://github.com/j178/prek)
- [prek Configuration Documentation](https://prek.j178.dev/configuration/)
- [prek Quickstart Guide](https://prek.j178.dev/quickstart/)

### タスクランナー

- [just — command runner](https://github.com/casey/just)
- [just-mcp — AI agent integration for justfiles](https://github.com/toolprint/just-mcp)
- [Task Runners — Rust Project Primer](https://rustprojectprimer.com/tools/tasks.html)

### リリース

- [release-plz GitHub](https://github.com/release-plz/release-plz)
- [Fully Automated Releases for Rust — Orhun's Blog](https://blog.orhun.dev/automated-rust-releases/)
- [cargo-release](https://github.com/crate-ci/cargo-release)

### CI

- [sccache in GitHub Actions](https://depot.dev/blog/sccache-in-github-actions)
- [Optimizing CI Pipeline for Rust](https://jwsong.github.io/blog/ci-optimization/)
- [Optimizing CI/CD Pipelines for Rust (LogRocket)](https://blog.logrocket.com/optimizing-ci-cd-pipelines-rust-projects/)
- [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache)

### エラーハンドリング

- [thiserror vs anyhow vs snafu comparison](https://dev.to/leapcell/rust-error-handling-compared-anyhow-vs-thiserror-vs-snafu-2003)

### 型駆動開発

- [Make Illegal States Unrepresentable — corrode Rust Consulting](https://corrode.dev/blog/illegal-state/)
- [RAII Guards and Newtypes in Rust](https://benjamincongdon.me/blog/2025/12/23/RAII-Guards-and-Newtypes-in-Rust/)
- [Type-Driven Development in Rust — ruggero.io](https://www.ruggero.io/blog/rust_type_driven_development_guide/)
- [Clean Architecture with Rust — DeepWiki](https://deepwiki.com/flosse/clean-architecture-with-rust/2-architecture)

### LLMコード品質

- [Evaluating LLM-based Agents on Rust Issue Resolution (arXiv 2026)](https://arxiv.org/html/2602.22764v1)
- [An AI agent coding skeptic tries AI agent coding — Max Woolf](https://minimaxir.com/2026/02/ai-agent-coding/)
- [Agentic Engineering Code Review Guardrails — Propel](https://www.propelcode.ai/blog/agentic-engineering-code-review-guardrails)
- [AI Agent Guardrails: Production Guide for 2026 — Authority Partners](https://authoritypartners.com/insights/ai-agent-guardrails-production-guide-for-2026/)

### コンパイル時アサーション

- [Compile-time assertions — timClicks](https://timclicks.dev/tip/compile-time-assertions)
- [static_assertions crate](https://docs.rs/static_assertions/latest/static_assertions/)
