# g3proxy

The g3proxy is an enterprise level forward proxy, but still with basic support for
tcp streaming / tls streaming / transparent proxy / reverse proxy.

## User Guide

[中文](UserGuide.zh_CN.md) | [English](UserGuide.en_US.md)

## Features

### Server

- **General**

  * Ingress network filter | Target Host filter | Target Port filter
  * Socket Speed Limit | Request Rate Limit | IDLE Check
  * Protocol Inspection | TLS Interception | ICAP Adaptation (experimental)
  * Various TCP & UDP socket config options
  * Rustls TLS Server
  * Openssl/BoringSSL/AWS-LC/Tongsuo TLS Server & Client
  * Tongsuo TLCP Server & Client (国密《GB/T 38636-2020》)

- **Forward Proxy**

  - Http(s) Proxy

    * TLS / mTLS
    * Http Forward | Https Forward | Http CONNECT | Ftp over HTTP
    * Basic User Authentication
    * Port Hiding

  - Socks Proxy

    * Socks4 Tcp Connect | Socks5 Tcp Connect | Socks5 UDP Associate
    * User Authentication
    * Client side UDP IP Binding / IP Map / Ranged Port

- **Transparent Proxy**

  - SNI Proxy

    * Multiple Protocol: TLS SNI extension | HTTP Host Header
    * Host Redirection / Host ACL

  - TCP TPROXY

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

  - Plain TCP Port
    * PROXY Protocol
  - Plain TLS Port
    * PROXY Protocol
    * mTLS
    * based on Rustls
  - Native TLS Port
    * PROXY Protocol
    * mTLS
    * based on OpenSSL/BoringSSL/AWS-LC/Tongsuo
  - Intelli Proxy
    * Multiple protocol: Http Proxy | Socks Proxy
    * PROXY Protocol

### Escaper

- **General**

  * Happy Eyeballs
  * Socket Speed Limit
  * Various TCP & UDP socket config options
  * IP Bind

- **Direct Connect**

  - DirectFixed

    * TCP Connect | TLS Connect | HTTP(s) Forward | UDP Associate
    * Egress network filter
    * Resolve redirection
    * Index based Egress Path Selection

  - DirectFloat

    * TCP Connect | TLS Connect | HTTP(s) Forward | UDP Associate
    * Egress network filter
    * Resolve redirection
    * Dynamic IP Bind
    * Json based Egress Path Selection

- **Proxy Chaining**

  - Http Proxy

    * TCP Connect | TLS Connect | HTTP(s) Forward
    * PROXY Protocol
    * Load Balance: RR / Random / Rendezvous / Jump Hash
    * Basic User Authentication

  - Https Proxy

    * TCP Connect | TLS Connect | HTTP(s) Forward
    * PROXY Protocol
    * Load Balance: RR / Random / Rendezvous / Jump Hash
    * Basic User Authentication
    * mTLS

  - Socks5 Proxy

    * TCP Connect | TLS Connect | HTTP(s) Forward | UDP Associate
    * Load Balance: RR / Random / Rendezvous / Jump Hash
    * Basic User Authentication

  - ProxyFloat

    * Dynamic Proxy: Http Proxy | Https Proxy | Socks5 Proxy
    * Json based Egress Path Selection

#### Router

- route-client - based on client addresses
  * exact ip match
  * subnet match
- route-mapping - based on user supplied rules in requests
  * Index based Egress Path Selection
- route-query - based on queries to external agent
- route-resolved - based on resolved IP of target host
- route-geoip - based on GeoIP rules if the resolved IP
- route-select - simple load balancer
  * RR / Random / Rendezvous / Jump Hash
  * Json based Egress Path Selection
- route-upstream - based on original target host
  * exact ip match
  * exact domain match
  * wildcard domain match
  * subnet match
  * regex domain match
- route-failover - failover between primary and standby escaper

### Resolver

- c-ares
  * UDP
  * TCP
- hickory
  * UDP / TCP
  * DNS over TLS
  * DNS over HTTPS
  * DNS over HTTP/3
  * DNS over QUIC
- fail-over

### Auth

- **User Authentication and Authorization**

  - ACL: Proxy Request | Target Host | Target Port | User Agent
  - Socket Speed Limit | Request Rate Limit | Request Alive Limit | IDLE Check
  - Auto Expire | Block
  - Anonymous user
  - Json based Egress Path Selection
  - Explicit Site Config
    * match by exact ip | exact domain | wildcard domain | subnet
    * request | client traffic | remote traffic metrics
    * task duration histogram metrics

### Audit

- TCP Protocol Inspection
- Task Level Sampling
- TLS Interception
- External TLS Certificate Generator
- TLS Stream Dump
- Http1 & Http2 Interception
- ICAP Adaptation

### Logging

- Log Types
  * Server: task log
  * Escaper: escape error log
  * Resolver: resolve error log
  * Audit: inspect & intercept log
- Backend: journald | syslog | fluentd

### Metrics

- Metrics Types
  * Server level metrics
  * Escaper level metrics
  * User level metrics
  * User-Site level metrics
- Protocol: StatsD

## Documents

The detailed docs are resided in the [doc](doc) directory.

## Examples

You can find example config in the [examples](examples) directory.
