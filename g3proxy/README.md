# g3proxy

The g3proxy is an enterprise level forward proxy, but still with basic support for
tcp streaming / tls streaming / transparent proxy / reverse proxy.

## User Guide

[中文版](UserGuide.zh_CN.md)

## Features

### Server

- **General**

  * Ingress network filter / Target Host filter / Target Port filter
  * Socket Speed Limit / Request Rate Limit / IDLE Check
  * Protocol Inspection / TLS Interception / ICAP Adaptation
  * Various TCP / UDP socket config options

- **Forward Proxy**

  - Http(s) Proxy

    * TLS / mTLS
    * Http Forward / Https Forward / Http CONNECT / Ftp over HTTP
    * Basic User Authentication
    * Port Hiding

  - Socks Proxy

    * Socks4 Tcp Connect / Socks5 Tcp Connect / Socks5 UDP Associate
    * User Authentication
    * Client side UDP IP Binding / IP Map / Ranged Port

- **Transparent Proxy**

  - SNI Proxy

    * Multiple Protocol: TLS SNI extension / HTTP Host Header
    * Host Redirection / Host ACL

- **Reverse Proxy**

  - Http(s) Reverse Proxy

    * TLS / mTLS
    * Basic User Authentication
    * Port Hiding
    * Host based Routing

- **Streaming**

  - TCP Stream

    * Upstream TLS / mTLS
    * Load Balance: RR / Random / Rendezvous / Jump Hash

  - TLS Stream

    * mTLS
    * Upstream TLS / mTLS
    * Load Balance: RR / Random / Rendezvous / Jump Hash

- **Alias Port**

  - TCP Port
  - TLS Port
    * mTLS
  - Intelli Proxy
    * Multiple protocol: Http Proxy / Socks Proxy

### Escaper

- **General**

  * Happy Eyeballs
  * Socket Speed Limit
  * Various TCP / UDP socket config options
  * IP Bind

- **Direct Connect**

  - Fixed

    * TCP Connect / TLS Connect / HTTP(s) Forward / UDP Associate
    * Egress network filter
    * Resolve redirection

  - Float

    * TCP Connect / TLS Connect / HTTP(s) Forward
    * Egress network filter
    * Resolve redirection
    * Dynamic IP Bind

- **Proxy Chaining**

  - Http Proxy

    * TCP Connect / TLS Connect / HTTP(s) Forward
    * PROXY Protocol
    * Load Balance: RR / Random / Rendezvous / Jump Hash
    * Basic User Authentication

  - Https Proxy

    * TCP Connect / TLS Connect / HTTP(s) Forward
    * PROXY Protocol
    * Load Balance: RR / Random / Rendezvous / Jump Hash
    * Basic User Authentication
    * mTLS

  - Socks5 Proxy

    * TCP Connect / TLS Connect / HTTP(s) Forward / UDP Associate
    * Load Balance: RR / Random / Rendezvous / Jump Hash
    * Basic User Authentication

  - Float

    * Dynamic Proxy: Http Proxy / Https Proxy / Socks5 Proxy (no UDP)

#### Router

- route-client - based on client addresses
  * exact ip match
  * subnet match
- route-mapping - based on user supplied rules in requests
- route-query - based on queries to external agent
- route-resolved - based on resolved IP of target host
- route-select - simple load balancer
  * RR / Random / Rendezvous / Jump Hash
- route-upstream - based on original target host
  * exact ip match
  * exact domain match
  * wildcard domain match
  * subnet match
  * regex domain match

### Resolver

- c-ares
  * UDP
  * TCP
- trust-dns
  * UDP / TCP
  * DNS over TLS
  * DNS over HTTPS
- fail-over

### Auth

- **User Authentication and Authorization**

  - ACL: Proxy Request / Target Host / Target Port / User Agent
  - Socket Speed Limit / Request Rate Limit / Request Alive Limit / IDLE Check
  - Auto Expire / Block
  - Explicit Site Config
    * match by exact ip / exact domain / wildcard domain / subnet

### Audit

- TCP Protocol Inspection
- TLS Interception
- Http / H2 Interception
- ICAP Adaptation & Sampling

### Logging

- Log Types
  * Server: task log
  * Escaper: escape error log
  * Resolver: resolve error log
  * Audit: inspect & intercept log
- Backend: journald / syslog / fluentd

### Metrics

- Metrics Types
  * Server level metrics
  * Escaper level metrics
  * User level metrics
  * User-Site level metrics
- Backend: statsd, so we can support multiple backends via statsd implementations

## Documents

The detailed docs are resided in the [doc](doc) directory.

## Examples

You can find example config in the [examples](examples) directory.
