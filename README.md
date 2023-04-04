[![minimum rustc: 1.66](https://img.shields.io/badge/minimum%20rustc-1.66-green?logo=rust)](https://www.whatrustisit.com)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache_2.0-blue.svg)](LICENSE)

# G3 Project

## About

This is the project we used to build enterprise-oriented general proxy solutions,
including but not limited to proxy / reverse proxy / load balancer / nat traversal.

## Components

G3 Project is make up of many components.

The project level docs resides in the *doc* subdirectory, and you should see the links below for the important ones.
Each component will have its own doc in its *doc* subdirectory.

### g3proxy

A general forward proxy solution, but you can also use it as tcp streaming / transparent proxy / reverse proxy
as we have basic support built in.

See [g3proxy](g3proxy/README.md) for detailed introduction.

### g3tiles

A work in progress reverse proxy solution.

### g3bench

A benchmark tool for the test of g3proxy.

### g3fcgen

Fake certificate generator for g3proxy.

## Dev-env Setup Guide

Follow [dev-setup](doc/dev-setup.md).

## Standards

Follow [standards](doc/standards.md).

## Release and Packaging

We will set tags for each release of each component, in the form *\<name\>-v\<version\>*.
You can use these tags to generate source tarballs.
And we have added deb and rpm package files for each component that is ready for distribution.

If you want to do a release build:

 1. generate a release tarball

    ```shell
    ./scripts/release/build_tarball.sh <name>-v<version>
    ```

    All vendor sources will be added to the source tarball, so you can save the source tarball and build it offline at
    anywhere that have the compiler and dependencies installed.

 2. build the package

    For deb package:
    ```shell
    tar xf <name>-<version>.tar.xz
    cd <name>-<version>
    ./build_deb_from_tar.sh
    ```

    For rpm package:
    ```shell
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
