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

- Generate the incorporating code

  ```shell
  ./scripts/generate/boringssl/incorporate.sh
  ```

- Build with `--features vendored-boringssl` cargo option

### Package

- Install a recent version of [go](https://go.dev/dl/), which is only needed if you want to

  * build packages directly from git
  * generate release tarball

- Switch to branch `rel/boringssl`, then run the build script or create the release tarball as usual.

- Copy the release tarball anywhere to build the final package, this doesn't require the go build dependency.

# AWS-LC

See [AWS-LC](https://github.com/aws/aws-lc) for more introduction.

## When

OpenSSL below 3.0 is not supported anymore, but the 3.0 - 3.2 versions all have significant performance degradation.
As an alternative, you can switch to use AWS-LC as a solution on AWS EC2 hosts.

## How

AWS-LC is supported in branch `rel/aws-lc`.

### Build

- Make sure you have `cmake`, `pkg-config` installed

- Install a recent version of [go](https://go.dev/dl/) if you want to do AWS-LC code generation.

- Build with `--features vendored-aws-lc` cargo build option.

### Package

Switch to branch `rel/aws-lc`, then run the build script or create the release tarball as usual.
