# g3bench

Benchmark tool for many targets.

## Features

### General

- PROXY Protocol
- Metrics
- mTLS / Rich TLS config options
- Socket Speed limit
- Progress Bar
- IP Bind

### Targets

- *HTTP 1.x*

  * GET / HEAD
  * Socks5 proxy / Http Proxy / Https Proxy

- *HTTP 2*

  * GET / HEAD
  * Socks5 proxy / Http Proxy / Https Proxy
  * Connection Pool

- *HTTP 3*

  * GET / HEAD
  * Connection Pool

- *TLS Handshake*

- *Cloudflare Keyless*

  * Connection Pool
  * Multiplex Connection / Simplex Connection

### Metrics

- Metrics Types
    * Target level metrics
- Backend: statsd, so we can support multiple backends via statsd implementations
