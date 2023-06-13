
Setup for g3proxy coverage tests
================================

# Install Required Tools

We use the following tools in the coverage scripts:

## docker

We use docker containers to run various target services, i.e. httpbin.

Install on Debian:
```shell
apt install docker.io
```

## dnsmasq

We use dnsmasq to add local dns records, and also use it as the target dns server.

You have 2 choices to run dnsmasq:

- NetworkManager Plugin

  If you have enabled dnsmasq plugin in NetworkManager, then there is nothing to do. The conf directory will be **/etc/NetworkManager/dnsmasq.d/**.

- Standalone dnsmasq Service

  If not, you have to install dnsmasq as a standalone service. The conf directory will be **/etc/dnsmasq.d/**.

  Install on Debian:
  ```text
  apt install dnsmasq
  ```

# Setup local DNS

## Modify /etc/hosts

Add the following lines to **/etc/hosts**:
```text
127.0.0.1 g3proxy.local
127.0.0.1 httpbin.local
```

Save the following conf file to **dnsmasq.d/02-add-hosts.conf**:

```text
addn-hosts=/etc/hosts
```

Then restart **NetworkManager** or **dnsmasq** which should respawn the real dnsmasq process.

# Run the Docker Containers

## httpbin

```shell
docker run -p 127.0.0.1:80:80 -d --name httpbin kennethreitz/httpbin
```

## vsftpd

```shell
mkdir /tmp/vsftpd
docker run -d -v /tmp/vsftpd:/home/vsftpd \
                -p 127.0.0.1:20:20 \
                -p 127.0.0.1:21:21 \
                -p 127.0.0.1:47400-47470:47400-47470 \
                -e FTP_USER=ftpuser \
                -e FTP_PASS=ftppass \
                -e PASV_ADDRESS=127.0.0.1 \
                --name ftp \
                -d bogem/ftp
```
