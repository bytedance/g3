[![minimum rustc: 1.86](https://img.shields.io/badge/minimum%20rustc-1.86-green?logo=rust)](https://www.whatrustisit.com)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache_2.0-blue.svg)](LICENSE)
[![codecov](https://codecov.io/gh/bytedance/g3/graph/badge.svg?token=TSQCA4ALQM)](https://codecov.io/gh/bytedance/g3)
[![docs](https://readthedocs.org/projects/g3-project/badge)](https://g3-project.readthedocs.io/)

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
- [scripts](scripts) には、カバレッジ テスト、パッケージ化スクリプト

### g3proxy

汎用のフォワードプロキシソリューションですが、TCPストリーミング / トランスペアレントプロキシ / リバースプロキシとしても使用できます。
基本的なサポートが組み込まれています。

#### 特徴のハイライト

- 非同期Rust: 高速で信頼性が高い
- Http1 / Socks5フォワードプロキシプロトコル、SNIプロキシおよびTCP TPROXY
- サポート easy-proxy & masque/http Well-Known URI
- プロキシチェイニング、上流プロキシの動的選択をサポート
- 多くの出口ルート選択方法、カスタム出口選択エージェントをサポート
- TCP/TLSストリームプロキシ、基本的なHTTPリバースプロキシ
- OpenSSL、BoringSSL、AWS-LC、AWS-LC-FIPS、Tongsuo、さらにはrustlsを使用したTLS
- TLS MITMインターセプション、復号化されたトラフィックダンプ、HTTP1/HTTP2/IMAP/SMTPインターセプション
- HTTP1/HTTP2/IMAP/SMTPのICAP適応、サードパーティのセキュリティ製品とシームレスに統合可能
- 優雅なリロード
- カスタマイズ可能なロードバランシングおよびフェイルオーバー戦略
- ユーザー認証、豊富な設定オプション
- 各ユーザーに対して差別化されたサイト設定を行うことが可能
- 豊富なACL/制限ルール、入口/出口/ユーザーレベルで
- 豊富な監視メトリクス、入口/出口/ユーザー/ユーザーサイトレベルで
- さまざまな観測ツールをサポート

[詳細な紹介](g3proxy/README.md) | [ユーザーガイド](g3proxy/UserGuide.en_US.md) |
[リファレンスドキュメント](https://g3-project.readthedocs.io/projects/g3proxy/en/latest/)

### g3statsd

StatsD互換の統計アグリゲータ。

[詳細な紹介](g3statsd/README.md) | [リファレンスドキュメント](https://g3-project.readthedocs.io/projects/g3statsd/en/latest/)

### g3tiles

作業中のリバースプロキシソリューション。

[リファレンスドキュメント](https://g3-project.readthedocs.io/projects/g3tiles/en/latest/)

### g3bench

HTTP 1.x、HTTP 2、HTTP 3、TLSハンドシェイク、DNS、Cloudflare Keylessをサポートするベンチマークツール。

[詳細な紹介](g3bench/README.md)

### g3mkcert

ルートCA / 中間CA / TLSサーバー / TLSクライアント / TLCPサーバー / TLCPクライアント 証明書を作成するツール。

[詳細な紹介](g3mkcert/README.md)

### g3fcgen

g3proxyのための偽の証明書ジェネレーター。

[詳細な紹介](g3fcgen/README.md)

### g3iploc

g3proxyのGeoIPサポートのためのIPロケーションルックアップサービス。

[詳細な紹介](g3iploc/README.md)

### g3keymess

Cloudflare keylessサーバーの簡単な実装。

[詳細な紹介](g3keymess/README.md) |
[リファレンスドキュメント](https://g3-project.readthedocs.io/projects/g3keymess/en/latest/)

## 対応プラットフォーム

現在、完全にサポートされているのはLinuxのみです。コードはFreeBSD、NetBSD、OpenBSD、macOS、Windowsでコンパイルされますが、そこでのテストは行っていません。

他のプラットフォームのサポートを追加するためのPRを歓迎します。

## 開発環境のセットアップガイド

[Dev-Setup](doc/dev-setup.md) に従ってください。

## 標準

[Standards](doc/standards.md) に従ってください。

## ビルド、パッケージ化、デプロイ

[Build and Package](doc/build_and_package.md) を参照してください。

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
