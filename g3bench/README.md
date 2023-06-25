# g3bench

Benchmark tool for many targets.

## Features

### General

- Metrics
- mTLS / Rich TLS config options
- Progress Bar
- IP Bind

### Targets

- *HTTP 1.x*

  * GET / HEAD
  * Socks5 proxy / Http Proxy / Https Proxy
  * PROXY Protocol
  * Socket Speed limit

- *HTTP 2*

  * GET / HEAD
  * Socks5 proxy / Http Proxy / Https Proxy
  * Connection Pool
  * PROXY Protocol
  * Socket Speed limit

- *HTTP 3*

  * GET / HEAD
  * Connection Pool

- *TLS Handshake*

  * PROXY Protocol

- *DNS*

  * DNS over UDP
  * DNS over TCP
  * DNS over TLS
  * DNS over HTTPS
  * DNS over QUIC

- *Cloudflare Keyless*

  * Connection Pool
  * Multiplex Connection / Simplex Connection

### Metrics

- Metrics Types
    * Target level metrics
- Backend: statsd, so we can support multiple backends via statsd implementations
