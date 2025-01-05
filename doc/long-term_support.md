Long-Term Support
-----

# Policy

We will offer LTS branches for applications that is considered to be production ready,
the branch name will be in the format **lts/<name>/<version>/<feature>**, such as *lts/g3proxy/1.10/default*.

LTS branches will only get bug & security fixes, so there won't be any new features or breaking changes.
The dependency lock file `Cargo.lock` will only get semver compatible updates when necessary.

Each LTS branch will be supported until 6 months after the next LTS branch for the same application.
You can ask for commercial support if you need a longer support time.

# Current LTS branches

## [g3proxy-v1.10](https://github.com/bytedance/g3/tree/lts/g3proxy/1.10/default)

Long-Term branch for [g3proxy](../g3proxy) 1.10.x.

Minimum requirements:

- MSRV: 1.80
- Linux OS: Debian 10 and CentOS 7.9.

## [g3proxy-v1.8](https://github.com/bytedance/g3/tree/lts/g3proxy/1.8/default)

Long-Term branch for [g3proxy](../g3proxy) 1.8.x.

Minimum requirements:

- MSRV: 1.75
- Linux OS: Debian 10 and CentOS 7.9.
