Setup for g3bench coverage tests
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

  If you have enabled dnsmasq plugin in NetworkManager, then there is nothing to do. The conf directory will be
  **/etc/NetworkManager/dnsmasq.d/**.

- Standalone dnsmasq Service

  If not, you have to install dnsmasq as a standalone service. The conf directory will be **/etc/dnsmasq.d/**.

  Install on Debian:
  ```text
  apt install dnsmasq
  ```

# Setup local DNS

Save the following conf file to **dnsmasq.d/g3proxy-ci.conf**:

```text
address=/httpbin.local/127.0.0.1
address=/g3proxy.local/127.0.0.1
```

Then restart **NetworkManager** or **dnsmasq** which should respawn the real dnsmasq process.

# Run the Docker Containers

## httpbin

```shell
docker run -p 127.0.0.1:80:80 -d --name httpbin kennethreitz/httpbin
```

## influxdb

1. Run the container

   ```shell
   docker pull influxdb:3-core
   docker run -p 127.0.0.1:8181:8181 --rm influxdb:3-core --node-id local --object-store=memory
   ```

2. Create the auth token

   ```shell
   curl -X POST http://127.0.0.1:8181/api/v3/configure/token/admin | jq ".token" -r
   ```

3. Export the auth token as variable `INFLUXDB3_AUTH_TOKEN`.

## graphite

```shell
docker pull graphiteapp/graphite-statsd:latest
docker run -p 127.0.0.1:2003:2003 --rm graphiteapp/graphite-statsd
```
