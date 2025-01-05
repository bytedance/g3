[![minimum rustc: 1.83](https://img.shields.io/badge/minimum%20rustc-1.83-green?logo=rust)](https://www.whatrustisit.com)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache_2.0-blue.svg)](LICENSE)
[![codecov](https://codecov.io/gh/bytedance/g3/graph/badge.svg?token=TSQCA4ALQM)](https://codecov.io/gh/bytedance/g3)
[![docs](https://readthedocs.org/projects/g3-project/badge)](https://g3-project.readthedocs.io/)

# G3 Project

[中文版 README](README.zh_CN.md) | [日本語 README](README.ja_JP.md)

## About

This is the project we used to build enterprise-oriented generic proxy solutions,
including but not limited to proxy / reverse proxy (WIP) / load balancer (TBD) / nat traversal (TBD).

## Applications

The G3 project is consisted of many applications, each of which has a separate subdirectory containing its own code,
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
- Proxy Chaining, with support for dynamic selection of upstream proxies
- Plenty of egress route selection methods, with support for custom egress selection agent
- TCP/TLS Stream Proxy, Basic HTTP Reverse Proxy
- TLS over OpenSSL or BoringSSL or AWS-LC or Tongsuo, and even rustls
- TLS MITM interception, decrypted traffic dump, HTTP1/HTTP2/IMAP/SMTP interception
- ICAP adaptation for HTTP1/HTTP2/IMAP/SMTP, can integrate seamlessly with 3rd-party security products
- Graceful reload
- Customizable load balancing and failover strategies
- User Auth, with a rich set of config options
- Can set differential site config for each user
- Rich ACL/Limit rules, at ingress / egress / user level
- Rich monitoring metrics, at ingress / egress / user / user-site level
- Support for a variety of observability tools

See [g3proxy](g3proxy/README.md) for detailed introduction and user guide docs.

You can view the g3proxy reference documentation generated by sphinx online at
[Read the Docs](https://g3-project.readthedocs.io/projects/g3proxy/en/latest/),
including detailed configuration format, log format, metrics definition, protocol definition, etc.

### g3tiles

A work in progress reverse proxy solution.

You can view the g3tiles reference documentation generated by sphinx online at
[Read the Docs](https://g3-project.readthedocs.io/projects/g3tiles/en/latest/),
including detailed configuration format, log format, metrics definition, etc.

### g3bench

A benchmark tool that supports HTTP 1.x, HTTP 2, HTTP 3, TLS Handshake, DNS and Cloudflare Keyless.

See [g3bench](g3bench/README.md) for detailed introduction.

### g3mkcert

A tool to make root CA / intermediate CA / TLS server / TLS client certificates.

### g3fcgen

Fake certificate generator for g3proxy.

### g3iploc

IP location lookup service for g3proxy GeoIP support.

### g3keymess

A simple implementation of Cloudflare keyless server.

## Target Platform

Only Linux is fully supported yet. The code will compile on FreeBSD, NetBSD, macOS and Windows, but we haven't tested it
there.

Feel free to open PRs to add support for other platforms.

## Dev-env Setup Guide

Follow [Dev-Setup](doc/dev-setup.md).

## Standards

Follow [Standards](doc/standards.md).

## Release and Packaging

We will set tags for each release of each application in the form *\<name\>-v\<version\>*.
You can use these tags to generate source tarballs.
And we have added deb and rpm package files for each application that is ready for distribution.

If you want to do a release build:

1. generate a release tarball

   ```shell
   # if we have a tag <name>-v<version>
   ./scripts/release/build_tarball.sh <name>-v<version>
   # if no tags usable, you need to specify the git revision (e.g. HEAD)
   ./scripts/release/build_tarball.sh <name> <rev>
   ```

   All vendor sources will be added to the source tarball, so you can save the source tarball and build it offline at
   anywhere that has the compiler and dependencies installed.

2. build the package

   For deb package:
   ```shell
   tar xf <name>-<version>.tar.xz
   cd <name>-<version>
   ./build_deb_from_tar.sh
   ```

   For rpm package:
   ```shell
   rpmbuild -ta ./<name>-<version>.tar.xz
   # if failed, you can run the following commands manually:
   tar xvf <name>-<version>.tar.xz ./<name>-<version>/<name>.spec
   cp <name>-<version>.tar.xz ~/rpmbuild/SOURCES/
   rpmbuild -ba ./<name>-<version>/<name>.spec
   ```

If you want to build a package directly from the git repo:

- For deb package:

  ```shell
  ./build_deb_from_git.sh <name>
  ```

- For rpm package:

  ```shell
  ./build_rpm_from_git.sh <name>
  ```

### Pre-Built Packages

It is recommended to build packages yourself if you want to install them in a production environment.

For testing purpose, we have built and uploaded some packages to
[cloudsmith](https://cloudsmith.io/~g3-oqh/repos/), you can find installation instructions there.

### Build Docker Image

You can find Dockerfile(s) under *docker* folder of each application. The build command will be like

```shell
# run this in the source root dir
docker build -f <app>/docker/debian.Dockerfile . -t <app>:<tag>
# build without the source code
docker build -f <app>/docker/debian.Dockerfile github.com/bytedance/g3 -t <app>:<tag>
# if you have a source tarball, you can also use the URL of that tarball
```

### Static Linking

See [Static Linking](doc/static-linking.md).

### Build with different OpenSSL variants

See [OpenSSL Variants](doc/openssl-variants.md).

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
