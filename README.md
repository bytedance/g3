[![minimum rustc: 1.86](https://img.shields.io/badge/minimum%20rustc-1.86-green?logo=rust)](https://www.whatrustisit.com)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache_2.0-blue.svg)](LICENSE)
[![codecov](https://codecov.io/gh/bytedance/g3/graph/badge.svg?token=TSQCA4ALQM)](https://codecov.io/gh/bytedance/g3)
[![docs](https://readthedocs.org/projects/g3-project/badge)](https://g3-project.readthedocs.io/)

# G3 Project

[中文版 README](README.zh_CN.md) | [日本語 README](README.ja_JP.md)

## About

This is the project we used to build enterprise-oriented generic proxy solutions,
including but not limited to proxy / reverse proxy (WIP) / load balancer (TBD) / nat traversal (TBD).

## Applications

The G3 project consists of many applications, each of which has a separate subdirectory containing its own code,
documentation, etc.

In addition to the application directories, there are some public directories:

- [doc](doc) Contains project-level documentation.
- [sphinx](sphinx) is used to generate HTML reference documents for each application.
- [scripts](scripts) Contains various auxiliary scripts, including coverage testing, packaging scripts, etc.

### g3proxy

A generic forward proxy solution, but you can also use it as tcp streaming / transparent proxy / reverse proxy
as we have basic support built in.

#### Feature highlights

- Async Rust: fast and reliable
- Http1 / Socks5 forward proxy protocol, SNI Proxy and TCP TPROXY
- easy-proxy Well-Known URI
- Proxy Chaining, with support for dynamic selection of upstream proxies
- Plenty of egress route selection methods, with support for custom egress selection agent
- TCP/TLS Stream Proxy, Basic HTTP Reverse Proxy
- TLS over OpenSSL / BoringSSL / AWS-LC / AWS-LC-FIPS / Tongsuo, and even rustls
- TLS MITM interception, decrypted traffic dump, HTTP1/HTTP2/IMAP/SMTP interception
- ICAP adaptation for HTTP1/HTTP2/IMAP/SMTP, can integrate seamlessly with 3rd-party security products
- Graceful reload
- Customizable load balancing and failover strategies
- User Auth, with a rich set of config options
- Can set differential site config for each user
- Rich ACL/Limit rules, at ingress / egress / user level
- Rich monitoring metrics, at ingress / egress / user / user-site level
- Support for a variety of observability tools

[README](g3proxy/README.md) | [User Guide](g3proxy/UserGuide.en_US.md) |
[Reference Doc](https://g3-project.readthedocs.io/projects/g3proxy/en/latest/)

### g3statsd

A StatsD compatible stats aggregator.

[README](g3statsd/README.md) | [Reference Doc](https://g3-project.readthedocs.io/projects/g3statsd/en/latest/)

### g3tiles

A work in progress reverse proxy solution.

[Reference Doc](https://g3-project.readthedocs.io/projects/g3tiles/en/latest/)

### g3bench

A benchmark tool that supports HTTP 1.x, HTTP 2, HTTP 3, TLS Handshake, DNS and Cloudflare Keyless.

[README](g3bench/README.md)

### g3mkcert

A tool to make root CA / intermediate CA / TLS server / TLS client / TLCP server / TLCP client certificates.

[README](g3mkcert/README.md)

### g3fcgen

Fake certificate generator for g3proxy.

[README](g3fcgen/README.md)

### g3iploc

IP location lookup service for g3proxy GeoIP support.

### g3keymess

A simple implementation of Cloudflare keyless server.

[README](g3keymess/README.md) |
[Reference Doc](https://g3-project.readthedocs.io/projects/g3keymess/en/latest/)

## Target Platform

Only Linux is fully supported yet. The code will compile on FreeBSD, NetBSD, OpenBSD, macOS and Windows, but we haven't
tested it there.

Feel free to open PRs to add support for other platforms.

## Dev-env Setup Guide

Follow [Dev-Setup](doc/dev-setup.md).

## Standards

Follow [Standards](doc/standards.md).

## Build, Package and Deploy

See [Build and Package](doc/build_and_package.md).

### LTS Version

See [Long-Term Support](doc/long-term_support.md).

## Contribution

Please check [Contributing](CONTRIBUTING.md) for more details.

## Code of Conduct

Please check [Code of Conduct](CODE_OF_CONDUCT.md) for more details.

## Security

If you discover a potential security issue in this project, or think you may
have discovered a security issue, we ask that you notify Bytedance Security via our
[security center](https://security.bytedance.com/src) or [vulnerability reporting email](mailto:sec@bytedance.com).

Please do **not** create a public GitHub issue.

## License

This project is licensed under the [Apache-2.0 License](LICENSE).

## 404Starlink

<img src="https://github.com/knownsec/404StarLink/raw/master/Images/logo.png" width="30%">

[g3proxy](g3proxy) has joined [404Starlink](https://github.com/knownsec/404StarLink)
