# Use Intel QAT Engine with g3keymess

## Install build dependencies

```shell
apt install cmake libssl-dev autoconf libtool pkgconf nasm
```

## Install crypto_mb

```shell
git clone https://github.com/intel/cryptography-primitives.git
cd cryptography-primitives/
git checkout v1.1.0 # checkout a released version
cd sources/ippcp/crypto_mb/
cmake -B build -DCMAKE_BUILD_TYPE=Release .
cd build/
make
make install
```

## Install intel-ipsec-mb

```shell
git clone https://github.com/intel/intel-ipsec-mb.git
cd intel-ipsec-mb/
cmake -B build -DCMAKE_BUILD_TYPE=Release .
cd build/
make
make install
```

## Install QAT Engine

### OpenSSL Engine

Supported since OpenSSL 1.1.1. Deprecated with OpenSSL 3.0.

```shell
git clone https://github.com/intel/QAT_Engine.git
cd QAT_Engine/
git checkout v1.9.0 # checkout a released version
./autogen.sh
./configure --enable-qat_sw --disable-qat_hw # change to what you want
make
make install
```

Verify:

```shell
openssl speed -engine qatengine -elapsed -async_jobs 8 rsa2048
```

Example openssl.cnf (/etc/ssl/qat-engine.cnf):

```text
openssl_conf = openssl_init

[openssl_init]
engines = engine_sect

[engine_sect]
qat = qat_sect

[qat_sect]
engine_id = qatengine
default_algorithms = ALL
```

### OpenSSL Provider

Available since OpenSSL 3.0.

```shell
git clone https://github.com/intel/QAT_Engine.git
cd QAT_Engine/
git checkout v1.9.0 # checkout a released version
./autogen.sh
./configure --enable-qat_provider --enable-qat_sw --disable-qat_hw # change to what you want
make
make install
```

Verify:

```shell
openssl speed -provider qatprovider -elapsed -async_jobs 8 rsa2048
```

Example openssl.cnf (/etc/ssl/qat-provider.cnf):

```text
openssl_conf = openssl_init

[openssl_init]
providers = provider_sect

[provider_sect]
default = default_sect
qat = qat_sect

[default_sect]
activate = 1

[qat_sect]
identity = qatprovider
activate = 1
```

## Example g3keymess config

Assume that the config directory is `/etc/g3keymess/test`, and the openssl.cnf file path is `/etc/ssl/qat.cnf`

then you need to have

`/etc/g3keymess/test/env` contains

```text
OPENSSL_CONF=/etc/ssl/qat.cnf
```

`/etc/g3keymess/test/main.yaml` can be written as:

- without worker (single core)

  ```yaml
  server:
    - name: default
      listen: "[::]:1300"
      # enable multiplex mode to use openssl async job
      multiplex_queue_depth: 128

  store:
    - name: local
      type: local
      dir: keys
  ```

- with worker (multiple cores)

  ```yaml
  worker:
    thread_number: 2

  backend: async_job # use openssl async job as backend driver

  server:
    - name: default
      listen: "[::]:1300"
      # enable multiplex mode to use workers
      multiplex_queue_depth: 128

  store:
    - name: local
      type: local
      dir: keys
  ```
