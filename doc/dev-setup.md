Dev-Setup
-----

# Toolchain

## Install rustup

See [rustup.rs](https://rustup.rs/) to install **rustup**.
It is recommended to use a non-root user.

*cargo*, *rustc*, *rustup* and other commands will be installed to Cargo's bin directory.
The default path is $HOME/.cargo/bin, and the following examples will use this.
You need to add this directory to your PATH environment variable.

- Bash

  The setup script should have already added the following line to your $HOME/.profile:
  ```shell script
  source "$HOME/.cargo/env"
  ```

- Fish

  Run the following command:
  ```shell script
  set -U fish_user_paths $HOME/.cargo/bin $fish_user_paths
  ```

## Update rustup

```shell script
rustup self update
```

## Install stable toolchains

List all available components:

```shell
rustup component list
```

The following components is required and should have already been installed:

- rustc
- rust-std
- cargo
- rustfmt
- clippy

**llvm-tools** is also recommended to be installed:

```shell script
rustup component add llvm-tools
```

## Install nightly toolchains

Install nightly toolchains:

```shell script
rustup toolchain install nightly
```

List components in nightly channel:

```shell script
rustup component list --toolchain nightly
```

## Update toolchains

Run the following command to update the toolchains for all channel:

```shell script
rustup update
```

# Plugins for cargo

To install:

```shell script
cargo install <crate name>
```

To update:

```shell script
cargo install -f <crate name>
```

The following plugins is recommended:

- cargo-expand

  Needed by IDE(at least JetBrains' rust plugin) to expand macros.
  The nightly toolchain is also required to run this.

- cargo-audit

  Audit Cargo.lock for crates with security vulnerabilities.

- cargo-binutils

  To run llvm-tools installed via rustup.

- cargo-cache

  To clean cargo caches.

# IDE

## JetBrains

There is an official [rust plugin](https://plugins.jetbrains.com/plugin/8182-rust) for JetBrains IDEs.

**PyCharm Community Edition** is recommended as we also use Python scripts in this repo.
**Clion** is needed if you want the **DEBUG** feature.

# Dependent Tools and Libraries

## Fast Install Guides

### Debian based Linux distribution

It is recommended to use Debian based distro as your development platform.

```shell
apt-get install gcc pkgconf make capnproto
apt-get install curl jq xz-utils tar
apt-get install libssl-dev libc-ares-dev
# install lua5.4 or any other versions available on your system
apt-get install lua5.4-dev
apt-get install libpython3-dev
apt-get install python3-toml python3-requests python3-pycurl python3-semver python3-socks python3-dnspyton
apt-get install python3-sphinx python3-sphinx-rtd-theme
apt-get install lsb-release dpkg-dev debhelper
```

### RHEL based Linux distribution

The devel packages is contained in repos that is not enabled by default,
you need to check the files under /etc/yum.repo.d/ and enable the corresponding repos.
See [EPEL Quickstart](https://docs.fedoraproject.org/en-US/epel/#_quickstart) for more info.

Some scripting or testing tools may be unavailable.

```shell
# enable epel repo first
dnf install epel-release
dnf update

#
dnf install gcc pkgconf make capnproto
dnf install curl jq xz tar
dnf install openssl-devel c-ares-devel lua-devel
dnf install python3-devel
dnf install python3-toml python3-requests python3-pycurl python3-semver
dnf install python3-sphinx python3-sphinx_rtd_theme
dnf install rpmdevtools rpm-build
```

### MacOS

```shell
brew install pkgconf capnp
brew install openssl c-ares
brew install lua
# install python, or you can use the one provided by XCode
brew install python
```

### Windows

```shell
# install rust toolchain
winget install Rustlang.Rust.MSVC
# install tools
winget install Kitware.CMake
winget install capnproto.capnproto
winget install NASM.NASM Ninja-build.Ninja
# install libraries
vcpkg install --triplet=x64-windows-static-md openssl
# build, c-ares need to be vendored, lua and python feature need to be disabled
cargo build --no-default-features --features quic,vendored-c-ares,hickory
```

**Tips**

- Install WinGET without `Windows App Store`:

  ```shell
  # Download the new release from https://github.com/microsoft/winget-cli/releases
  Add-AppxPackage -Path <xxx.msixbundle>
  ```

- Install a standalone version of `vcpkg`:

  ```shell
  git clone https://github.com/microsoft/vcpkg.git
  cd vcpkg
  .\bootstrap-vcpkg.bat
  # Then add the install path to `Path` and `VCPKG_ROOT` environment variable
  ```

### FreeBSD

```shell
pkg install rust
pkg install pkgconf capnproto
pkg install gmake # for vendored build of openssl
pkg install c-ares
# install lua5.4 or any other versions available on your system, and create a pkgconfig link
pkg install lua54
ln -s /usr/local/libdata/pkgconfig/lua-5.4.pc /usr/local/libdata/pkgconfig/lua5.4.pc
pkg install python3
# build, with vendored openssl
cargo build --features vendored-openssl
```

**Tips**

- Use the latest ports packages

  The default config in */etc/pkg/FreeBSD.conf* is configured to use quarterly pkg builds,
  you can run the following commands to switch to use the latest pkg builds:

  ```shell
  mkdir -p /usr/local/etc/pkg/repos/
  echo 'FreeBSD: {url: "pkg+http://pkg.FreeBSD.org/${ABI}/latest"}' > /usr/local/etc/pkg/repos/FreeBSD.conf
  pkg update -f
  pkg upgrade -y
  ```

### NetBSD

```shell
pkgin install pkgconf capnproto
pkgin install libcares
# install lua5.4 or any other versions available on your system, and create a pkgconfig link
pkgin install lua54
ln -s /usr/pkg/lib/pkgconfig/lua-5.4.pc /usr/pkg/lib/pkgconfig/lua5.4.pc
# install python 3.11 or any other versions available on your system, and create links
pkgin install python311
ln -s /usr/pkg/bin/python3.11 /usr/pkg/bin/python3
```

### OpenBSD

```shell
# install rust toolchain
pkg_add rust
# install capnproto from source
# install libs
pkg_add libcares
# install lua5.4 or any other versions available on your system, and create a pkgconfig link
pkg_add lua
ln -s /usr/local/lib/pkgconfig/lua54.pc /usr/local/lib/pkgconfig/lua5.4.pc
pkg_add python
# build, with vendored openssl
cargo build --vendored-openssl
```

**Tips**

- Increase process memory limit size

  The `datasize-cur` limit in `/etc/login.conf` for login class `staff` need to be increased if the compilation failed
  with error *out of memory*.

## Development Libraries

For *g3proxy*:

```text
openssl >= 1.1.1
c-ares >= 1.13.0
lua
python3 >= 3.7
```

## Development Tools

The tools for C development should be installed, including but not limited to:

```text
gcc
pkg-config
```

If the c-ares version in the OS repo is too old, the following tools is also required:

```text
cmake
```

## Rpc Code Generator

We use capnproto rpc to communicate with the running daemon process:

```text
capnproto
```

## Testing Tools

The following tools are needed to run testing scripts:

```text
curl
```

## Scripting Tools

The following tools are used in scripts under directory *scripts/*:

```text
git
jq
tar
xz
```

## Scripting Libraries

We use python3 for more complicated scripts, the following packages are needed:

```text
toml
requests
semver
PySocks
dnspython
```

## Document Tools

We use [sphinx](https://www.sphinx-doc.org/en/master/) to generate docs, with
theme [sphinx-rtd-theme](https://pypi.org/project/sphinx-rtd-theme/).

## Packaging Tools

### deb

For all *Debian* based distributions:

```text
lsb-release
dpkg-dev
debhelper
```

### rpm

For all *rhel* based distributions:

```text
rpmdevtools
rpm-build
```
