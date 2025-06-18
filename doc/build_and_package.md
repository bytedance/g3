Build, Package and Deploy
-----

**Table of Contents**

- [Linux Distribution Package](#linux-distribution-package)
    + [Build a Release Package](#build-a-release-package)
    + [Build a Snapshot Package](#build-a-snapshot-package)
    + [Pre-Built Packages](#pre-built-packages)
- [Docker Image](#docker-image)
- [Build Options](#build-options)
    + [Static Linking](#static-linking)
    + [OpenSSL Variants](#build-with-different-openssl-variants)

# Linux Distribution Package

`deb` and `rpm` package files are provided by default for each application, you can use them to build binary packages to
install on other machines.

## Build a Release Package

We set tags for each release of each application in the form *\<name\>-v\<version\>*.
You can use these tags to generate release source tarballs and then build packages from these tarballs.

1. generate a release tarball

   ```shell
   # if we have a tag <name>-v<version>
   ./scripts/release/build_tarball.sh <name>-v<version>
   # if no tags usable, you need to specify the git revision (e.g. HEAD)
   ./scripts/release/build_tarball.sh <name> <rev>
   ```

   All vendor sources will be added to the source tarball, so it can be build offline at anywhere that has the compiler
   and system dependency libraries installed.

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

## Build a Snapshot Package

Build directly from a git commit:

- Check out to the git commit first

- For deb package:

  ```shell
  ./build_deb_from_git.sh <name>
  ```

- For rpm package:

  ```shell
  ./build_rpm_from_git.sh <name>
  ```

Build from a snapshot source tarball:

- Generate the snapshot source tarball

  ```shell
  ./scripts/release/build_tarball.sh <name>-v<version>-<snapshot id> <git commit ID>
  ```

- Copy the generated source tarball to the target build machine and build

## Pre-Built Packages

It is recommended to build packages yourself if you want to install them in a production environment.

For testing purpose, we have built and uploaded some packages to
[cloudsmith](https://cloudsmith.io/~g3-oqh/repos/), you can find installation instructions there.

# Docker Image

You can find Dockerfile(s) under the *docker* folder of each application. The build command will be like

```shell
# run this in the source root dir
docker build -f <app>/docker/debian.Dockerfile . -t <app>:<tag>
# build without the source code
docker build -f <app>/docker/debian.Dockerfile github.com/bytedance/g3 -t <app>:<tag>
# if you have a source tarball, you can also use the URL of that tarball
```

# Build Options

## Static Linking

See [Static Linking](static-linking.md).

## Build with different OpenSSL variants

See [OpenSSL Variants](openssl-variants.md).
