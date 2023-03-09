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

- cargo-outdated

  Useful if you want to find out the outdated dependencies in your Cargo.toml.

- cargo-audit

  Audit Cargo.lock for crates with security vulnerabilities.
 
- cargo-license

  To see license of dependencies.

- cargo-binutils

  To run llvm-tools-preview installed via rustup.

# IDE

## JetBrains

There is an official [rust plugin](https://plugins.jetbrains.com/plugin/8182-rust) for JetBrains IDEs.

**PyCharm Community Edition** is recommended as we also use Python scripts in this repo.
**Clion** is needed if you want the **DEBUG** feature.

# Dependent Tools and Libraries

## Development Libraries

For *g3proxy*:
```text
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
llvm
mkcert
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

