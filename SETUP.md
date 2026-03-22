# 初期セットアップ

## 1. 開発環境

```bash
# devShell に入る（全ツールが自動で使える + git hooks も自動インストール）
nix develop
```

direnv を使う場合:

```bash
echo "use flake" > .envrc
direnv allow
```

## 2. GitHub リポジトリ設定

### Secrets

Settings → Secrets and variables → Actions で追加:

| Secret 名 | 取得元 | 用途 |
|---|---|---|
| `CARGO_REGISTRY_TOKEN` | [crates.io](https://crates.io/settings/tokens) → New Token → scope: `publish-new`, `publish-update` | release-plz が crates.io に publish |

### Actions 設定

Settings → Actions → General:

- [ ] **Allow GitHub Actions to create and approve pull requests** を有効化（release-plz がリリースPRを作成するために必要）

### Branch protection (推奨)

Settings → Branches → `main`:

- [ ] Require status checks to pass before merging → `check`, `test`, `audit` を必須に
- [ ] Require branches to be up to date before merging

### CODEOWNERS (推奨)

```bash
# .github/CODEOWNERS
/.github/workflows/ @ryo-morimoto
```

## 3. 外部サービス連携（任意）

| サービス | 用途 | セットアップ |
|---|---|---|
| [Codecov](https://codecov.io) | カバレッジ可視化 | GitHub App インストール → リポジトリ接続。tokenless（OIDC）で動くので Secret 追加不要 |
| [CodSpeed](https://codspeed.io) | ベンチマーク回帰検出 | 未設定でもベンチは動く。連携するなら divan を `codspeed-divan-compat` に差し替え |

## 4. 動作確認

```bash
# 全チェックが通ることを確認
just ci

# ベンチマークが動くことを確認
just bench
```
