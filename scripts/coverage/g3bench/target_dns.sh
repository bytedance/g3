
# Dns over UDP, via Cloudflare Public DNS
g3bench dns "1.1.1.1" www.example.com,A --dump-result

# Dns over TCP, via Cloudflare Public DNS
g3bench dns "1.1.1.1" --tcp www.example.com,A --dump-result

# Dns over TLS, via Cloudflare Public DNS
g3bench dns "1.1.1.1" -e dot www.example.com,A --dump-result

# Dns over Https, via Cloudflare Public DNS
g3bench dns "1.1.1.1" -e doh www.example.com,A --dump-result

# Dns over Quic, via AdGuard Public DNS
g3bench dns "94.140.14.140" -e doq www.example.com,A --dump-result

# Dns over Http/3, via AdGuard Public DNS
g3bench dns "94.140.14.140" -e doh3 www.example.com,A --dump-result
