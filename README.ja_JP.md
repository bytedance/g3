[![minimum rustc: 1.83](https://img.shields.io/badge/minimum%20rustc-1.83-green?logo=rust)](https://www.whatrustisit.com)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache_2.0-blue.svg)](LICENSE)
[![codecov](https://codecov.io/gh/bytedance/g3/graph/badge.svg?token=TSQCA4ALQM)](https://codecov.io/gh/bytedance/g3)

# G3プロジェクト

[中文版 README](README.zh_CN.md) | [English README](README.md)

## 概要

これは、エンタープライズ向けの汎用プロキシソリューションを構築するために使用されるプロジェクトです。
プロキシ / リバースプロキシ（作業中） / ロードバランサー（未定） / NATトラバーサル（未定）などを含むがこれらに限定されません。

## アプリ

G3 プロジェクトは多数のアプリケーションで構成されており、各アプリケーションには独自のコード、ドキュメントなどを含む個別のサブディレクトリがあります。

アプリ ディレクトリに加えて、いくつかのパブリック ディレクトリもあります。

- [doc](doc) プロジェクトレベルのドキュメントが含まれます。
- [sphinx](sphinx) は、各アプリの HTML リファレンス ドキュメントを生成するために使用されます。
- [scripts](scripts) には、カバレッジ テスト、パッケージ化スクリ

### g3proxy

汎用のフォワードプロキシソリューションですが、TCPストリーミング / トランスペアレントプロキシ / リバースプロキシとしても使用できます。
基本的なサポートが組み込まれています。

#### 特徴のハイライト

- 非同期Rust: 高速で信頼性が高い
- Http1 / Socks5フォワードプロキシプロトコル、SNIプロキシおよびTCP TPROXY
- プロキシチェイニング、上流プロキシの動的選択をサポート
- 多くの出口ルート選択方法、カスタム出口選択エージェントをサポート
- TCP/TLSストリームプロキシ、基本的なHTTPリバースプロキシ
- OpenSSL、BoringSSL、AWS-LC、Tongsuo、さらにはrustlsを使用したTLS
- TLS MITMインターセプション、復号化されたトラフィックダンプ、HTTP1/HTTP2/IMAP/SMTPインターセプション
- HTTP1/HTTP2/IMAP/SMTPのICAP適応、サードパーティのセキュリティ製品とシームレスに統合可能
- 優雅なリロード
- カスタマイズ可能なロードバランシングおよびフェイルオーバー戦略
- ユーザー認証、豊富な設定オプション
- 各ユーザーに対して差別化されたサイト設定を行うことが可能
- 豊富なACL/制限ルール、入口/出口/ユーザーレベルで
- 豊富な監視メトリクス、入口/出口/ユーザー/ユーザーサイトレベルで
- さまざまな観測ツールをサポート

詳しい機能紹介とユーザーガイドは[g3proxy](g3proxy/README.md)をご覧ください。

Sphinx を使用して生成された g3proxy リファレンス
ドキュメントは、[Read the Docs](https://g3-project.readthedocs.io/projects/g3proxy/en/latest/)
でオンラインで参照できます。 詳細な設定形式、ログ形式、メトリクスの定義、メトリクスの定義などを含みます。

### g3tiles

作業中のリバースプロキシソリューション。

Sphinx を使用して生成された g3tiles リファレンス
ドキュメントは、[Read the Docs](https://g3-project.readthedocs.io/projects/g3tiles/en/latest/)
でオンラインで参照できます。
詳細な設定形式、ログ形式、メトリクスの定義などを含みます。

### g3bench

HTTP 1.x、HTTP 2、HTTP 3、TLSハンドシェイク、DNS、Cloudflare Keylessをサポートするベンチマークツール。

詳細な紹介については、[g3bench](g3bench/README.md) を参照してください。

### g3mkcert

ルートCA / 中間CA / TLSサーバー / TLSクライアント証明書を作成するツール。

### g3fcgen

g3proxyのための偽の証明書ジェネレーター。

### g3iploc

g3proxyのGeoIPサポートのためのIPロケーションルックアップサービス。

### g3keymess

Cloudflare keylessサーバーの簡単な実装。

## 対応プラットフォーム

現在、完全にサポートされているのはLinuxのみです。コードはFreeBSD、NetBSD、macOS、Windowsでコンパイルされますが、そこでのテストは行っていません。

他のプラットフォームのサポートを追加するためのPRを歓迎します。

## 開発環境のセットアップガイド

[Dev-Setup](doc/dev-setup.md) に従ってください。

## 標準

[Standards](doc/standards.md) に従ってください。

## リリースとパッケージング

各アプリの各リリースには *\<name\>-v\<version\>* の形式でタグが設定されます。
これらのタグを使用してソースtarballを生成できます。
また、配布準備が整った各アプリにはdebおよびrpmパッケージファイルが追加されています。

リリースビルドを行う場合:

1. リリースtarballを生成する

   ```shell
   # <name>-v<version> のタグがある場合
   ./scripts/release/build_tarball.sh <name>-v<version>
   # 使用可能なタグがない場合、gitリビジョン（例: HEAD）を指定する必要があります
   ./scripts/release/build_tarball.sh <name> <rev>
   ```

   すべてのベンダーソースはソースtarballに追加されるため、ソースtarballを保存し、コンパイラと依存関係がインストールされている任意の場所でオフラインでビルドできます。

2. パッケージをビルドする

   debパッケージの場合:
   ```shell
   tar xf <name>-<version>.tar.xz
   cd <name>-<version>
   ./build_deb_from_tar.sh
   ```

   rpmパッケージの場合:
   ```shell
   rpmbuild -ta ./<name>-<version>.tar.xz
   # 失敗した場合、次のコマンドを手動で実行できます:
   tar xvf <name>-<version>.tar.xz ./<name>-<version>/<name>.spec
   cp <name>-<version>.tar.xz ~/rpmbuild/SOURCES/
   rpmbuild -ba ./<name>-<version>/<name>.spec
   ```

gitリポジトリから直接パッケージをビルドする場合:

- debパッケージの場合:

  ```shell
  ./build_deb_from_git.sh <name>
  ```

- rpmパッケージの場合:

  ```shell
  ./build_rpm_from_git.sh <name>
  ```

### 事前ビルドパッケージ

本番環境にインストールする場合は、自分でパッケージをビルドすることをお勧めします。

テスト目的の場合、いくつかのパッケージをビルドして
[cloudsmith](https://cloudsmith.io/~g3-oqh/repos/) にアップロードしました。インストール手順はそこにあります。

### Dockerイメージのビルド

各アプリの *docker* フォルダーの下にDockerfile(s)があります。ビルドコマンドは次のようになります

```shell
# ソースルートディレクトリで実行します
docker build -f <app>/docker/debian.Dockerfile . -t <app>:<tag>
# ソースコードなしでビルドします
docker build -f <app>/docker/debian.Dockerfile github.com/bytedance/g3 -t <app>:<tag>
# ソースtarballがある場合、そのtarballのURLも使用できます
```

### 静的リンク

[Static Linking](doc/static-linking.md) を参照してください。

### 異なるOpenSSLバリアントでのビルド

[OpenSSL Variants](doc/openssl-variants.md) を参照してください。

### LTSバージョン

[Long-Term Support](doc/long-term_support.md) を参照してください。

## 貢献

詳細については [Contributing](CONTRIBUTING.md) を参照してください。

## 行動規範

詳細については [Code of Conduct](CODE_OF_CONDUCT.md) を参照してください。

## セキュリティ

このプロジェクトで潜在的なセキュリティ問題を発見した場合、またはセキュリティ問題を発見したと思われる場合は、
[セキュリティセンター](https://security.bytedance.com/src) または [脆弱性報告メール](mailto:sec@bytedance.com)
を通じてBytedance Securityに通知してください。

公開のGitHub issueを作成しないでください。

## ライセンス

このプロジェクトは [Apache-2.0 License](LICENSE) の下でライセンスされています。
