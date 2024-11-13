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

# BoringSSL

See [BoringSSL](https://boringssl.googlesource.com/boringssl/) for more introduction.

## When

OpenSSL below 3.0 is not supported anymore, but the 3.0 - 3.2 versions all have significant performance degradation.
As an alternative, you can switch to use BoringSSl as a solution.

## How

### Build

- Make sure you have `cmake`, `pkg-config`installed

- Build with `--features vendored-boringssl` cargo option

# AWS-LC

See [AWS-LC](https://github.com/aws/aws-lc) for more introduction.

## When

OpenSSL below 3.0 is not supported anymore, but the 3.0 - 3.2 versions all have significant performance degradation.
As an alternative, you can switch to use AWS-LC as a solution on AWS EC2 hosts.

## How

### Build

- Make sure you have `cmake`, `pkg-config` installed

- Install a recent version of [go](https://go.dev/dl/) if you want to do AWS-LC code generation.

- Build with `--no-default-features --features vendored-aws-lc,rustls-aws-lc,<other features>` cargo build option.
