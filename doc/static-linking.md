# Static Linking

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
cargo build --target=x86_64-unknown-linux-musl --no-default-features --features vendored-openssl,c-ares
```
