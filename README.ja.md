[English](README.md) | [日本語](README.ja.md)

<p align="center">
  <b>E A G R A P H</b>
</p>

<img width="1783" height="940" alt="image" src="https://github.com/user-attachments/assets/5273145f-6814-46da-9e65-34893526bd85" />

<p align="center">
  <sub><i>ALCHEMISTA Labs コードグラフ</i></sub>
</p>

---

eagraphは、Claude Codeがコードナビゲーションに消費するトークンを削減するためのコード知識グラフです。エージェントが呼び出し元の特定、コールチェーンの追跡、ファイル構造の把握を行う際、通常はgrep、glob、readを何度も連鎖させる必要があります。eagraphは事前構築済みのインデックスに対する1回のクエリで同じ情報を返します。Claude Codeスキルとして提供されるほか、スタンドアロンCLIとしても動作し、コードベースをブラウザで閲覧できるインタラクティブなグラフビジュアライザーも含まれています。
Rust製。パースにtree-sitter、ストレージにSQLiteを使用。全データはOSのアプリケーションディレクトリに保存され、リポジトリ内には一切書き込みません。


以下は、大規模コードベースであるTiny C Compilerのソースコードを探索した例です。プロンプトはソースファイルから実行可能ファイル出力までのコンパイル実行パスを追跡する内容です。`eagraph`スキル（左側）と`eagraph-explorer`エージェントを使用することで、ツール呼び出し回数、トークン使用量、探索時間を大幅に削減できました。関数の行番号を含むより正確な結果も得られています。結果は環境により異なります。

<img width="1783" height="940" alt="image" src="https://github.com/user-attachments/assets/5273145f-6814-46da-9e65-34893526bd85" />

## インストール

[最新リリース](https://github.com/eaglys-shared/eagraph/releases/latest)からビルド済みバイナリをダウンロードできます。タグごとに以下のターゲットが公開されます。

- `x86_64-unknown-linux-gnu`: Linux x86_64
- `aarch64-apple-darwin`: macOS Apple Silicon

Intel Mac向けビルドは提供されていません。Intel Macユーザーはソースからビルドしてください（下記参照）。

各リリースページにはターゲットごとの `.tar.gz` と `.tar.gz.sha256` ファイルが掲載されています。両方をダウンロードし、アーカイブを検証してからバイナリをインストールしてください。

```bash
# リリースページからtarballとsha256ファイルをダウンロードしてください
tar -xzf eagraph-v<X.Y.Z>-<target>.tar.gz
shasum -a 256 -c eagraph-v<X.Y.Z>-<target>.tar.gz.sha256
sudo install eagraph-v<X.Y.Z>-<target>/eagraph /usr/local/bin/
```

### macOS: 未署名バイナリについて

macOS向けビルドは**コード署名・公証されていません**。初回実行時にGatekeeperが「"eagraph"は開発元を確認できないため開けません」または「実行可能ファイルが壊れています」というエラーを表示します。検疫属性を削除して実行を許可してください。

```bash
xattr -d com.apple.quarantine /usr/local/bin/eagraph
```

インストール後に一度だけ実行すれば完了します。未署名バイナリを使いたくない場合は、ソースからビルドしてください。

### ソースからビルド

安定版のRustツールチェーンが必要です（[rustup](https://rustup.rs)からインストール）。

```bash
cargo build --release -p eagraph-cli
sudo install target/release/eagraph /usr/local/bin/
```

## Claude Codeでの使用

eagraphは主にClaude Codeスキルとして使用されます。CLIをPATHに配置したら、`skill/` と `agent/` ディレクトリをClaude Codeの設定にコピーしてください。どちらもリリースtarball内にバイナリと一緒に含まれています。ソースからビルドした場合は、リポジトリのルートにあります。

```bash
mkdir -p ~/.claude/skills ~/.claude/agents
cp -r skill ~/.claude/skills/eagraph
cp agent/eagraph-explorer.md ~/.claude/agents/
```

アップグレード時は、新しいtarballを展開した後（またはソースをpullした後）にこれらのコピーを再実行してください。スキルはClaude Codeに `eagraph context`、`eagraph symbols` などの使用方法を教え、複数のgrep/glob/read呼び出しの連鎖を不要にします。エージェントはサブエージェントにもeagraphの使用を優先させます。

Claude Codeが異なるインストールパスを指定している場合は、そちらに従ってください。上記のコマンドは標準的なUnixのパスです。

## CLIとしての使用

```bash
# 言語のgrammarをインストール
eagraph grammars add python typescript rust go

# セットアップ
eagraph init myorg
eagraph add /path/to/my-project

# クエリ
eagraph query MyClass
eagraph context MyClass
eagraph dependents src/models.py
eagraph symbols src/models.py
eagraph chain function_a function_b

# ビジュアライズ
eagraph viz
```

`eagraph add` は言語を検出し、不足しているgrammarを推薦し、即座にインデックスを作成します。リポジトリはgitリポジトリである必要があります。

## コマンド

2つのグループがあります。クエリコマンド（`query`、`context`、`dependents`、`symbols`、`chain`）はClaude Codeスキルがエージェントの代わりに実行するもので、残りは管理・セットアップ用です。全てのクエリコマンドは `--json` で構造化出力に対応しています。`--repo` はカレントディレクトリから自動検出されます。

| コマンド | 説明 |
|---|---|
| `eagraph init <org>` | 設定ファイルを作成 |
| `eagraph add <path> [--name X]` | リポジトリを追加、言語検出、自動インデックス |
| `eagraph index <repo> [--force] [--all]` | リポジトリをインデックス |
| `eagraph status` | リポジトリ、ブランチ、シンボル数を表示 |
| `eagraph query <name>` | シンボルを名前で検索 |
| `eagraph context <symbol> [--depth N]` | シンボルの近傍とソーススニペット |
| `eagraph dependents <file> [--depth N]` | ファイルに依存するものを表示 |
| `eagraph symbols <file>` | ファイルの目次 |
| `eagraph chain <from> <to>` | 2つのシンボル間の最短呼び出しパス |
| `eagraph viz [--port N]` | ブラウザでインタラクティブグラフを表示 |
| `eagraph config` | 設定パスとgrammarパスを表示 |
| `eagraph grammars add <lang>...` | grammarをコンパイル・インストール |
| `eagraph grammars list` | インストール済み・利用可能なgrammarを表示 |
| `eagraph grammars check` | リポジトリに推奨されるgrammarを表示 |

データは常に最新です。全てのクエリがファイルのmtimeをチェックし、古くなったファイルを自動的に再インデックスします。

## Grammar

```bash
eagraph grammars add python typescript rust go java
```

各grammarのリポジトリをクローンし、共有ライブラリにコンパイルしてインストールします。Cコンパイラ（`cc`）が必要です。

利用可能な全grammarの一覧: `eagraph grammars list`

<details>
<summary>未登録のgrammarを手動でビルドする方法</summary>

```bash
git clone https://github.com/tree-sitter/tree-sitter-python
cd tree-sitter-python/src

# macOS
cc -shared -dynamiclib -fPIC -O2 -I. parser.c scanner.c -o python.dylib

# Linux
cc -shared -fPIC -O2 -I. parser.c scanner.c -o python.so
```

`.so`/`.dylib` を `.scm`（クエリパターン）と `.toml`（拡張子設定）と一緒にgrammarディレクトリに配置してください。`grammars/python.scm` と `grammars/python.toml` を参考にしてください。

</details>

## テストの実行

```bash
cargo test --workspace
```

ビルドスクリプトがベンダー化されたCソースからPython grammarの `.so` をコンパイルし、テストが `dlopen` を通じてそれを読み込みます。本番環境と同じ動的ローディングパスを検証します。

## プロジェクト構成

```
crates/
  eagraph-core/             型、トレイト、設定、エラー
  eagraph-store-sqlite/     GraphStore + SQLファイル
  eagraph-parser/           汎用tree-sitterエクストラクタ、動的grammarローディング
  eagraph-retriever/        コンテキストリトリーバー、スニペットリーダー
  eagraph-cli/              全コマンドを含むバイナリ
  eagraph-crossref/         クロスリポジトリ解決（スタブ）
  eagraph-mcp/              MCPサーバー（スタブ）
grammars/                   言語ごとの .scm + .toml、registry.toml
skill/                      Claude Codeスキル
agent/                      Claude Codeサブエージェント定義
tests/fixtures/
  grammars-src/             テスト用.soコンパイルのためのベンダー化Cソース
  sample-repo/              Pythonフィクスチャプロジェクト
```
