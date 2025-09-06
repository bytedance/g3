#!/usr/bin/env bash
set -euo pipefail

# 1) Add host loopback aliases (one-time per boot; idempotent)
for ip in 10.10.0.2 10.10.0.3 10.10.0.4; do
  if ! ifconfig lo0 | grep -q "inet ${ip} "; then
    sudo ifconfig lo0 alias ${ip}/32
  fi
done

# 2) Route *.test to CoreDNS on localhost:5300
sudo mkdir -p /etc/resolver
printf "nameserver 127.0.0.1\nport 5300\n" | sudo tee /etc/resolver/test >/dev/null
sudo dscacheutil -flushcache || true
sudo killall -HUP mDNSResponder 2>/dev/null || true

# 3) Bring up containers
docker compose up -d

echo "Ready.

Test DNS:
  dscacheutil -q host -a name myservice.test

Test TCP:
  nc -vz myservice.test 80
  nc -vz myservice.test 1080

Test HTTP proxy (use system curl):
  /usr/bin/curl -v -x http://myservice.test:80 http://ipinfo.io

Test SOCKS5 proxy (use system curl):
  /usr/bin/curl -v --socks5-hostname myservice.test:1080 http://ipinfo.io

(Ensure your 3 local proxies are listening on 8081/8082/8083.)"
