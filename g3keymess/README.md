[![docs](https://readthedocs.org/projects/g3-project-g3keymess/badge)](https://g3-project.readthedocs.io/projects/g3keymess/)

# g3keymess

g3keymess is a server implementation of Cloudflare Keyless protocol.

## How to build

You need to follow the [dev-setup](../doc/dev-setup.md) guide to set up your build environment first.

To build debug binaries:

```shell
cargo build -p g3keymess -p g3keymess-ctl
```

To build release binaries:

```shell
cargo build --profile release-lto -p g3keymess -p g3keymess-ctl
```

See [Packaging](../doc/packaging.md) if you want to build binary packages or docker images.

## Features

g3keymess uses the system OpenSSL by default.

You can specify the following feature flags to try others:

- vendored-openssl

  Use the latest OpenSSL.

- vendored-boringssl

  Use BoringSSL.

- vendored-tongsuo

  Use Tongsuo.

### Hardware Acceleration

It's possible to use hardware crypto engines by using
[OpenSSl ENGINES](https://github.com/openssl/openssl/blob/master/README-ENGINES.md) or
[OpenSSL PROVIDERS](https://github.com/openssl/openssl/blob/master/README-PROVIDERS.md).

Use the following compilation feature flags to enable OpenSSL Async Job support:

```text
cargo build --features openssl-async-job
```

You can build a hardware engine against the system OpenSSL, and enable it
in [openssl.cnf](https://docs.openssl.org/master/man5/config/). If you don't want
to change the default openssl.cnf, you can create a new one and export it as environment variable `OPENSSL_CONF`.

See [Intel QAT Engine](IntelQatEngine.md) for more detailed installation steps.

## Examples

See [examples](examples) directory.
