#!/usr/bin/env bash
set -euo pipefail

docker compose down -v

# remove host aliases
for ip in 10.10.0.2 10.10.0.3 10.10.0.4; do
  sudo ifconfig lo0 -alias ${ip} || true
done

sudo rm -f /etc/resolver/test
sudo dscacheutil -flushcache || true
sudo killall -HUP mDNSResponder 2>/dev/null || true

echo "Cleaned up."
