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

**llvm-tools-preview** and **rust-src** is also recommended being installed:
```shell script
rustup component add llvm-tools-preview
rustup component add rust-src
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

  To run llvm-tools-preview installed via rustup.

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
apt-get install gcc pkgconf libtool make capnproto
apt-get install curl jq xz-utils tar
apt-get install libssl-dev libc-ares-dev
# install lua5.4 or any other versions available on your system
apt-get install lua5.4-dev
apt-get install python3-dev
apt-get install python3-toml python3-requests python3-semver python3-socks python3-dnspyton
apt-get install python3-sphinx
apt-get install lsb-release dpkg-dev debhelper
apt-get --no-install-recommends devscripts
```

### RHEL based Linux distribution

The devel packages is contained in repos that is not enabled by default,
you need to check the files under /etc/yum.repo.d/ and enable the corresponding repos.
Some scripting or testing tools may be unavailable.

```shell
# enable epel repo first
dnf install epel-release
dnf update

#
dnf install gcc pkgconf libtool make capnproto
dnf install curl jq xz tar
dnf install openssl-devel c-ares-devel lua-devel
dnf install python3-devel
dnf install python3-toml python3-requests python3-semver
dnf install python3-sphinx
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

### FreeBSD

```shell
pkg install pkgconf capnproto
pkg install openssl c-ares
# install lua5.4 or any other versions available on your system, and create a pkgconfig link
pkg install lua54
ln -s /usr/local/libdata/pkgconfig/lua-5.4.pc /usr/local/libdata/pkgconfig/lua5.4.pc
pkg install python3
```

### NetBSD

```shell
pkgin install pkgconf libtool autoconf automake capnproto
pkgin install openssl libcares
# install lua5.4 or any other versions available on your system, and create a pkgconfig link
pkgin install lua54
ln -s /usr/pkg/lib/pkgconfig/lua-5.4.pc /usr/pkg/lib/pkgconfig/lua5.4.pc
# install python 3.10 or any other versions available on your system, and create links
pkgin install python310
ln -s /usr/pkg/lib/pkgconfig/python-3.10.pc /usr/pkg/lib/pkgconfig/python3.pc
ln -s /usr/pkg/bin/python3.10 /usr/pkg/bin/python3
```

## Development Libraries

For *g3proxy*:
```text
openssl
c-ares
lua
python3
```

## Development Tools

The tools for C development should be installed, including but not limited to:
```text
gcc
pkg-config
```

If the c-ares version in the OS repo is too old, the following tools is also required:
```text
libtool
make
```

## Rpc Code Generator

We use capnproto rpc in *g3proxy*:
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

We use [sphinx](https://www.sphinx-doc.org/en/master/) to generate docs.

## Packaging Tools

### deb

For all *Debian* based distributions:
```text
lsb-release
devscripts
dpkg-dev
debhelper
```

### rpm

For all *rhel* based distributions:
```text
rpmdevtools
rpm-build
```
