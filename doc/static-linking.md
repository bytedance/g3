Static Linking
---

# Linux

## Install musl

[musl](https://musl.libc.org/) is needed to enable static linking.

You can use the following instructions to install musl on Debian:

```shell
apt install musl-tools
```

## Install rustc target

Then you need to install the corresponding musl rust target (see `rustc --print target-list | grep musl`):

```shell
rustup target add x86_64-unknown-linux-musl
```

## Compile

Then compile with the features that do not require dynamic linking:

```shell
cargo build --target=x86_64-unknown-linux-musl --no-default-features --features vendored-openssl,rustls-ring,quic,vendored-c-ares
```

# Windows

Windows provides both dynamic and static C runtimes.

See [C runtime (CRT) and C++ standard library (STL) .lib files](https://learn.microsoft.com/en-us/cpp/c-runtime-library/crt-library-features).

You can change to use a static runtime by setting `-C target-feature=+crt-static` rustc flag.

See [Static and dynamic C runtimes](https://doc.rust-lang.org/reference/linkage.html#static-and-dynamic-c-runtimes).

## Compile with vcpkg

The environment variable `VCPKGRS_TRIPLET` need to be set to `x64-windows-static` first.

```shell
vcpkg install --triplet=x64-windows-static openssl c-ares
cargo build --no-default-features --features rustls-ring,quic,c-ares
```

## Compile without vcpkg

```shell
cargo build --no-default-features --features vendored-openssl,rustls-ring,quic,vendored-c-ares
```
