[![minimum rustc: 1.70](https://img.shields.io/badge/minimum%20rustc-1.70-green?logo=rust)](https://www.whatrustisit.com)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache_2.0-blue.svg)](LICENSE)

# G3 Project

[English README](README.md)

## 关于

本项目用于构建面向企业的通用代理解决方案，包括但不限于代理、反向代理（开发中）、负载均衡（待定）、NAT穿透（待定）等。

## 组件

G3 Project 由若干组件构成。

项目级的文档放在 [doc](doc) 子目录下，下文会列出一下比较重要的文档。
每个组件都有各自的文档，放在各自目录的 *doc* 子目录下。

### g3proxy

通用正向代理解决方案，同时带有TCP映射、TLS卸载/封装、透明代理、简单反向代理等功能。

更多详情参考 [g3proxy](g3proxy/README.md)。 

### g3tiles

通用反向代理解决方案，开发中。

### g3bench

压测工具，支持 HTTP/1.x、HTTP/2、HTTP/3、TLS握手、Cloudflare Keyless 。

更多详情参考 [g3bench](g3bench/README.md)。

#### g3mkcert

用来生成 根CA / 中间CA / TLS服务端证书 / TLS客户端证书 的工具。

### g3fcgen

适用于g3proxy TLS劫持功能的的伪造证书生成服务组件。

## 支持平台

目前仅提供对Linux系统的完整支持，其他系统如FreeBSD、NetBSd、MacOS可以编译，但是未测试过功能。

如果需要支持其他系统，欢迎提交PR。

## 开发环境搭建

参考 [dev-setup](doc/dev-setup.md)。

## 标准及约定

参考 [standards](doc/standards.md)。

## 发布及打包

每个组件的每个发布版本都会有对应的tag，格式为 *\<name\>-v\<version\>* 。
使用对应的tag生成源码tar包，该tar包可以用于生成deb、rpm等发行版原生包文件。

如果需要对正式发布的版本打包:

 1. 生成版本发布包

    ```shell
    ./scripts/release/build_tarball.sh <name>-v<version>
    ```

    所有引用第三方源码都会放在tar包的vendor目录下，打包时只需要在目标机器上安装好编译器及系统依赖库即可，无需额外的网络连接。

 2. 打包指令

    deb包:
    ```shell
    tar xf <name>-<version>.tar.xz
    cd <name>-<version>
    ./build_deb_from_tar.sh
    ```

    rpm包:
    ```shell
    rpmbuild -ta ./<name>-<version>.tar.xz
    # 如果失败，可以手动执行以下指令：
    tar xvf <name>-<version>.tar.xz ./<name>-<version>/<name>.spec
    cp <name>-<version>.tar.xz ~/rpmbuild/SOURCES/
    rpmbuild -ba ./<name>-<version>/<name>.spec
    ```

如果需要直接从git打包:

 - deb包:

   ```shell
   ./build_deb_from_git.sh <name>
   ```

 - rpm包:

   ```shell
   ./build_rpm_from_git.sh <name>
   ```

## 贡献指南

参考 [Contributing](CONTRIBUTING.md)。

## 交流合作

请使用[飞书](https://www.feishu.cn/download)加群，
[G3代理用户交流群加入链接](https://applink.feishu.cn/client/chat/chatter/add_by_link?link_token=9fah8def-d024-4db5-91cd-522ae09c2b72)，
或使用如下二维码:

<img alt="" src="https://github.com/bytedance/g3/blob/master/G3-FEISHU-USER-GROUP.png" width="50%" height="50%">

## Code of Conduct

Please check [Code of Conduct](CODE_OF_CONDUCT.md) for more details.

## Security

If you discover a potential security issue in this project, or think you may
have discovered a security issue, we ask that you notify Bytedance Security via our
[security center](https://security.bytedance.com/src) or [vulnerability reporting email](mailto:sec@bytedance.com).

Please do **not** create a public GitHub issue.

## License

This project is licensed under the [Apache-2.0 License](LICENSE).
