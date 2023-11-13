# g3bench

Benchmark tool for HTTP 1.x / HTTP 2 / HTTP 3 / TLS Handshake / DNS / Cloudflare Keyless.

## Features

### General

- Metrics
- mTLS / Rich TLS config options
- Progress Bar
- IP Bind

### Targets

- *HTTP 1.x*

  * GET / HEAD
  * Socks5 Proxy / Http Proxy / Https Proxy
  * PROXY Protocol
  * Socket Speed limit and IO stats (HTTP layer)
  * 国密《GB/T 38636-2020》（TLCP）(require feature vendored-tongsuo)

- *HTTP 2*

  * GET / HEAD
  * Socks5 Proxy / Http Proxy / Https Proxy
  * Connection Pool
  * PROXY Protocol
  * Socket Speed limit and IO stats (H2 layer)
  * 国密《GB/T 38636-2020》（TLCP）(require feature vendored-tongsuo)

- *HTTP 3*

  * GET / HEAD
  * Socks5 Proxy
  * Connection Pool
  * Socket Speed limit and IO stats (QUIC layer)

- *TLS Handshake*

  * PROXY Protocol
  * 国密《GB/T 38636-2020》（TLCP）(require feature vendored-tongsuo)

- *DNS*

  * DNS over UDP
  * DNS over TCP
  * DNS over TLS
  * DNS over HTTPS
  * DNS over HTTP/3
  * DNS over QUIC

- *Cloudflare Keyless*

  * Connection Pool
  * Multiplex Connection / Simplex Connection
  * 国密《GB/T 38636-2020》（TLCP）(require feature vendored-tongsuo)

### Metrics

- Metrics Types
    * Target level metrics
- Protocol: StatsD

# Examples

## Test a Http Server

```shell
# http, 100 concurrency, for 20 seconds
g3bench h1 http://example.net/echo1k -t 20s -c 100
# https, 100 concurrency, for 20 seconds
g3bench h1 https://example.net/echo1k -t 20s -c 100
# https, no keep-alive, 100 concurrency, for 20 seconds
g3bench h1 https://example.net/echo1k -t 20s -c 100 --no-keepalive
# using TLS 1.2 cipher ECDHE-RSA-AES256-GCM-SHA384
g3bench h1 https://example.net/echo1k -t 20s -c 100 --tls-protocol tls1.2 --tls-ciphers ECDHE-RSA-AES256-GCM-SHA384
# h2
g3bench h2 https://www.example.net
# h3
g3bench h3 https://www.example.net
```

## Test a Http Proxy

```shell
# using HTTP Forward, 100 concurrency, for 20 seconds
g3bench h1 -x http://192.168.1.1:3128 http://example.net/echo1k -t 20s -c 100
# using HTTPS Forward, 100 concurrency, for 20 seconds
g3bench h1 -x http://192.168.1.1:3128 https://example.net/echo1k -t 20s -c 100
# using HTTP CONNECT, 100 concurrency, for 20 seconds
g3bench h1 -x http://192.168.1.1:3128 -p https://example.net/echo1k -t 20s -c 100
# disable HTTP Keep-Alive
g3bench h1 -x http://192.168.1.1:3128 http://example.net/echo1k --no-keepalive -t 20s -c 100
# using FTP over HTTP
g3bench h1 -x http://192.168.1.1:3128 ftp://example.net/
# using HTTP CONNECT for h2
g3bench h2 -x http://192.168.1.1:3128 https://example.net
```

## Test DNS

```shell
# DNS over TLS, via Cloudflare Public DNS
g3bench dns "1.1.1.1" -e dot www.example.com,A --dump-result
# DNS over HTTPS, via Cloudflare Public DNS
g3bench dns "1.1.1.1" -e doh www.example.com,A --dump-result
# DNS over Quic, via AdGuard Public DNS
g3bench dns "94.140.14.140" -e doq www.example.com,A --dump-result
g3bench dns "2a10:50c0::1:ff" -e doq --tls-name unfiltered.adguard-dns.com www.example.com,A --dump-result
```
