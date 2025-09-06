# Sticky HTTP/SOCKS Env

This directory provides a small, local environment to simulate three upstream proxies behind a single hostname for the `examples/sticky_http_socks` demo. It sets up:

- `myservice.test` DNS that round-robins to `10.10.0.2`, `10.10.0.3`, `10.10.0.4` via CoreDNS.
- Loopback IP aliases for those addresses on your host.
- Port-forwarding containers that expose `10.10.0.{2,3,4}:80` and forward to host ports `8081/8082/8083`.

Use this to point `g3proxy` at multiple upstream proxies as defined in `examples/sticky_http_socks/g3proxy.yaml` and verify sticky routing behavior.

## What’s here

- `Corefile`: CoreDNS config mapping `myservice.test` to three IPs and forwarding other DNS to public resolvers.
- `docker-compose.yaml`: Runs CoreDNS and three minimal forwarders for port 80 → host `8081/8082/8083` and port 1080 → host `1081/1082/1083`.
- `setup.sh`: Adds loopback IP aliases, configures macOS resolver for `*.test` to use CoreDNS on `127.0.0.1:5300`, and starts the stack.
- `teardown.sh`: Stops containers and reverts resolver and loopback alias changes.
- `usage.sh`: Quick-start snippet showing common commands.

## Prerequisites

- macOS (the scripts use `lo0` aliases and `/etc/resolver`).
- Docker + Docker Compose v2 (`docker compose`).
- `sudo` privileges (to add loopback aliases and write `/etc/resolver/test`).
- Three local upstream proxies listening on host ports `8081`, `8082`, `8083` (HTTP), and optionally SOCKS5 proxies on `1081`, `1082`, `1083` if you want to test the SOCKS path.

## Quick start

1) Make scripts executable and run setup (prompts for sudo):

```bash
cd examples/sticky_http_socks/env
chmod +x setup.sh teardown.sh
./setup.sh
```

2) Verify DNS and connectivity:

```bash
# DNS should show multiple A records
/usr/bin/dscacheutil -q host -a name myservice.test

# TCP connectivity to the virtual VIP
nc -vz myservice.test 80
nc -vz myservice.test 1080
```

3) Exercise via HTTP proxy (round-robin across 3 backends):

```bash
/usr/bin/curl -v -x http://myservice.test:80 http://ipinfo.io
/usr/bin/curl -v -x http://myservice.test:80 http://ipinfo.io
/usr/bin/curl -v -x http://myservice.test:80 http://ipinfo.io
```

You should observe requests distributed across the three upstreams connected at `8081/8082/8083`.

## Using with g3proxy

- Sample config: `examples/sticky_http_socks/g3proxy.yaml` already references:
  - HTTP upstreams: `10.10.0.2:80`, `10.10.0.3:80`, `10.10.0.4:80` (backed by the port-forwarders → `8081/8082/8083`).
  - SOCKS5 upstreams: `10.10.0.2:1080`, `10.10.0.3:1080`, `10.10.0.4:1080`.
- For HTTP, no extra changes are needed if your three HTTP proxies are on `8081/2/3`.
- For SOCKS5, this env already forwards `10.10.0.{2,3,4}:1080` → host `1081/1082/1083`; run your local SOCKS5 servers on those host ports to match the example config.

Start `g3proxy` with the example config and test via its HTTP/SOCKS listeners as desired.

## Cleanup

When finished:

```bash
./teardown.sh
```

This stops containers, removes loopback aliases, and restores resolver settings.

## Notes

- The resolver setup targets the `test` TLD only (`/etc/resolver/test`), keeping normal DNS unaffected.
- CoreDNS listens on `127.0.0.1:5300`; other domains are forwarded to public resolvers (`1.1.1.1`, `8.8.8.8`).
- If ports `8081/8082/8083` are busy, adjust the compose file and your upstreams to matching ports.
- If ports `1081/1082/1083` are busy, update both the compose forwarders and your local SOCKS5 servers accordingly.

### Troubleshooting

- Error `socat: not found` in container logs: the image installs `socat` at startup via `apk`. If the install fails (e.g., no internet), the container exits. Ensure Docker has outbound internet or prebuild an image with `socat` included. As a quick check:
  - Run `docker compose pull` and retry `docker compose up -d`.
  - Verify your network/proxy settings allow Alpine packages to download.
