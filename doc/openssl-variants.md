OpenSSL Variants
-----

There are many forks of OpenSSL available, this doc will show you how to build with them.

# Tongsuo

See [Tongsuo](https://github.com/Tongsuo-Project/Tongsuo) for more introduction.

## When

If you want to support the following protocols:

- GB/T 38636-2020, TLCP
- RFC 8998, TLS1.3 + SM2

then you need to use Tongsuo.

## How

### Build

Use `--features vendored-tongsuo` cargo build option.

### Package

Switch to branch `rel/tlcp-tongsuo`, then run the build script or create the release tarball as usual.

# BoringSSL

See [BoringSSL](https://boringssl.googlesource.com/boringssl/) for more introduction.

## When

OpenSSL below 3.0 is not supported anymore, but the 3.0 - 3.2 versions all have significant performance degradation.
As an alternative, you can switch to use BoringSSl as a solution.

## How

BoringSSL is supported in branch `rel/boringssl`.

### Build

- Checkout the `boringssl` submodule

  ```shell
  git submodule init
  git submodule update
  ```

- Make sure you have `cmake`, `pkg-config` and `go` installed

- Install bindgen

  ```shell
  cargo install bindgen-cli
  ```

- Set `CC` and `CXXFLAGS` to the environment variable if you want

  you may find `-Wno-error=attributes` helpful.

- Get the target triple:

  ```shell
  TARGET_TRIPLE=$(cargo -V -v | awk '$1 == "host:" {print $2}')
  ```

- Build BoringSSL

  - Build with Makefile:
  
    ```shell
    cmake -DRUST_BINDINGS=$TARGET_TRIPLE -B boringssl/build/ -S boringssl/ -DCMAKE_BUILD_TYPE=Release
    cd boringssl/build
    make
    cd -
    ```

  - Build with Ninja:
    ```shell
    cmake -DRUST_BINDINGS=$TARGET_TRIPLE -B boringssl/build/ -S boringssl/ -DCMAKE_BUILD_TYPE=Release -GNinja
    cd boringssl/build
    ninja
    cd -
    ```

- Build with `--features vendored-boringssl` cargo option

### Package

You can build packages by using the build scripts after  BoringSSL is built
in `boringssl/build` directory.

Release tarball creation is still not supported yet.

# AWS-LC

See [AWS-LC](https://github.com/aws/aws-lc) for more introduction.

## When

OpenSSL below 3.0 is not supported anymore, but the 3.0 - 3.2 versions all have significant performance degradation.
As an alternative, you can switch to use AWS-LC as a solution on AWS EC2 hosts.

## How

AWS-LC is supported in branch `rel/boringssl`.

### Build

Use `--features vendored-aws-lc` cargo build option.

### Package

- Install a recent version of [go](https://go.dev/dl/).

- Switch to branch `rel/aws-lc`, then run the build script or create the release tarball as usual.
