Standards
---------

This file contains all the standards we have draw attention to during the development.
The code should comply to these, but should be more compliant to existing popular implementations.

# General

## URI

 - [rfc3986](https://datatracker.ietf.org/doc/html/rfc3986)
    : Uniform Resource Identifier (URI): Generic Syntax
 - [URL](https://url.spec.whatwg.org/)
    : Living Standard
 - [rfc1738](https://datatracker.ietf.org/doc/html/rfc1738)
    : Uniform Resource Locators (URL)

## Prefixes for Binary Multiples

 - [IEEE 1541-2002](https://en.wikipedia.org/wiki/IEEE_1541-2002)
    : IEEE Standard for Prefixes for Binary Multiples

## Date and Time

 - [rfc3339](https://datatracker.ietf.org/doc/html/rfc3339)
    : Date and Time on the Internet: Timestamps

## UUID

 - [rfc4122](https://datatracker.ietf.org/doc/html/rfc4122)
    : A Universally Unique IDentifier (UUID) URN Namespace

## Encoding

 - [netstring](http://cr.yp.to/proto/netstrings.txt)

## Syslog

 - [rfc3164](https://datatracker.ietf.org/doc/html/rfc3164)
    : The BSD syslog Protocol
 - [rfc5424](https://datatracker.ietf.org/doc/html/rfc5424)
    : The Syslog Protocol
 - [CEE Log Syntax](https://cee.mitre.org/language/1.0-beta1/cls.html)
    : CEE Log Syntax (CLS) Specification
 - [CEE Log Transport](https://cee.mitre.org/language/1.0-beta1/clt.html)
    : CEE Log Transport (CLT) Specification

## Fluentd

 - [Forward-Protocol-Specification-v1](https://github.com/fluent/fluentd/wiki/Forward-Protocol-Specification-v1)
    : Forward Protocol Specification v1

## PEN

 - [PRIVATE ENTERPRISE NUMBERS](https://www.iana.org/assignments/enterprise-numbers/enterprise-numbers)

## IP Address

 - [rfc6890](https://datatracker.ietf.org/doc/html/rfc6890)
    : Special-Purpose IP Address Registries
 - [rfc4291](https://datatracker.ietf.org/doc/html/rfc4291)
    : IP Version 6 Addressing Architecture
 - [rfc8215](https://datatracker.ietf.org/doc/html/rfc8215)
    : Local-Use IPv4/IPv6 Translation Prefix

## Semantic Versioning

 - [semver](https://semver.org/)
   : Semantic Versioning 2.0.0

## X.509

 - [rfc7468](https://datatracker.ietf.org/doc/html/rfc7468)
   : Textual Encodings of PKIX, PKCS, and CMS Structures
 - [rfc5280](https://datatracker.ietf.org/doc/html/rfc5280)
   : Internet X.509 Public Key Infrastructure Certificate and Certificate Revocation List (CRL) Profile
 - [rfc5758](https://datatracker.ietf.org/doc/html/rfc5758)
   : Internet X.509 Public Key Infrastructure: Additional Algorithms and Identifiers for DSA and ECDSA
 - [rfc4055](https://datatracker.ietf.org/doc/html/rfc4055/)
   : Additional Algorithms and Identifiers for RSA Cryptography for use in the Internet X.509 Public Key Infrastructure Certificate and Certificate Revocation List (CRL) Profile

## Cryptography

 - [NIST SP 800-186](https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-186.pdf)
   : Recommendations for Discrete Logarithm-based Cryptography: Elliptic Curve Domain Parameters
 - [SEC2-v2](https://www.secg.org/sec2-v2.pdf)
   : SEC 2: Recommended Elliptic Curve Domain Parameters

# Network Protocol

## Happy Eyeballs

 - [rfc8305](https://datatracker.ietf.org/doc/html/rfc8305)
   : Happy Eyeballs Version 2: Better Connectivity Using Concurrency

## PROXY protocol

 - [haproxy-proxy-protocol](https://www.haproxy.org/download/1.8/doc/proxy-protocol.txt)
   : The PROXY protocol Versions 1 & 2

## Socks

### Socks4

 - [SOCKS4](http://ftp.icm.edu.pl/packages/socks/socks4/SOCKS4.protocol)
    : SOCKS: A protocol for TCP proxy across firewalls
 - [SOCKS4a](https://www.openssh.com/txt/socks4a.protocol)
    : SOCKS 4A: A  Simple Extension to SOCKS 4 Protocol

### Socks5

 - [rfc1928](https://datatracker.ietf.org/doc/html/rfc1928)
    : SOCKS Protocol Version 5
 - [rfc1929](https://datatracker.ietf.org/doc/html/rfc1929)
    : Username/Password Authentication for SOCKS V5
 - [rfc1961](https://datatracker.ietf.org/doc/html/rfc1961)
    : GSS-API Authentication Method for SOCKS Version 5
 - [draft-ietf-aft-socks-chap-01](https://datatracker.ietf.org/doc/html/draft-ietf-aft-socks-chap-01)
    : Challenge-Handshake Authentication Protocol for SOCKS V5

### Socks6

 - [draft-olteanu-intarea-socks-6-11](https://datatracker.ietf.org/doc/html/draft-olteanu-intarea-socks-6-11)
    : SOCKS Protocol Version 6

## DNS

 - [rfc2181](https://datatracker.ietf.org/doc/html/rfc2181)
    : Clarifications to the DNS Specification
 - [rfc4343](https://datatracker.ietf.org/doc/html/rfc4343)
    : Domain Name System (DNS) Case Insensitivity Clarification
 - [draft-madi-dnsop-udp4dns-00](https://datatracker.ietf.org/doc/id/draft-madi-dnsop-udp4dns-00.html)
    : UDP payload size for DNS messages
 - [rfc5625](https://datatracker.ietf.org/doc/html/rfc5625)
    : DNS Proxy Implementation Guidelines
 - [rfc5891](https://datatracker.ietf.org/doc/html/rfc5891)
    : Internationalized Domain Names in Applications (IDNA): Protocol
 - [rfc6891](https://datatracker.ietf.org/doc/html/rfc6891)
    : Extension Mechanisms for DNS (EDNS(0))
 - [rfc6761](https://datatracker.ietf.org/doc/html/rfc6761)
    : Special-Use Domain Names
 - [rfc7858](https://datatracker.ietf.org/doc/html/rfc7858)
    : Specification for DNS over Transport Layer Security (TLS)
 - [rfc8484](https://datatracker.ietf.org/doc/html/rfc8484)
    : DNS Queries over HTTPS (DoH)
 - [rfc9250](https://datatracker.ietf.org/doc/html/rfc9250)
    : DNS over Dedicated QUIC Connections
 - [iana-domains-reserved](https://www.iana.org/domains/reserved)
    : IANA-managed Reserved Domains

## SSH

 - [rfc4253](https://datatracker.ietf.org/doc/html/rfc4253)
    : The Secure Shell (SSH) Transport Layer Protocol

## TLS

 - [rfc8446](https://datatracker.ietf.org/doc/html/rfc8446)
    : The Transport Layer Security (TLS) Protocol Version 1.3

 - [GB/T 38636-2020](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=778097598DA2761E94A5FF3F77BD66DA)
    : Information security technology—Transport layer cryptography protocol(TLCP)

## HTTP

 - [rfc9110](https://datatracker.ietf.org/doc/html/rfc9110)
    : HTTP Semantics
 - [rfc9111](https://datatracker.ietf.org/doc/html/rfc9111)
    : HTTP Caching
 - [mozilla-http](https://developer.mozilla.org/en-US/docs/Web/HTTP)
    : Web technology for developers - HTTP
 - [rfc7617](https://datatracker.ietf.org/doc/html/rfc7617)
    : The 'Basic' HTTP Authentication Scheme
 - [rfc7239](https://datatracker.ietf.org/doc/html/rfc7239)
    : Forwarded HTTP Extension
 - [iana-http-methods](https://www.iana.org/assignments/http-methods)
    : Hypertext Transfer Protocol (HTTP) Method Registry
 - [iana-http-status-codes](https://www.iana.org/assignments/http-status-codes/http-status-codes)
    : Hypertext Transfer Protocol (HTTP) Status Code Registry
 - [iana-http-fields](https://www.iana.org/assignments/http-fields/http-fields.xhtml)
    : Hypertext Transfer Protocol (HTTP) Field Name Registry
 - [mozilla-http-headers](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers)
    : HTTP headers
 - [rfc6648](https://datatracker.ietf.org/doc/html/rfc6648)
    : Deprecating the "X-" Prefix and Similar Constructs in Application Protocols
 - [rfc9297](https://datatracker.ietf.org/doc/html/rfc9297)
    : HTTP Datagrams and the Capsule Protocol
 - [rfc9298](https://datatracker.ietf.org/doc/html/rfc9298)
    : Proxying UDP in HTTP
 - [iana-http-upgrade-tokens](https://www.iana.org/assignments/http-upgrade-tokens/http-upgrade-tokens.xhtml)
    : Hypertext Transfer Protocol (HTTP) Upgrade Token Registry
 - [iana-well-known-uris](https://www.iana.org/assignments/well-known-uris/well-known-uris.xhtml)
    : Well-Known URIs

### HTTP/1.0

 - [rfc1945](https://datatracker.ietf.org/doc/html/rfc1945)
    : Hypertext Transfer Protocol -- HTTP/1.0

### HTTP/1.1

 - [rfc9112](https://datatracker.ietf.org/doc/html/rfc9112)
    : HTTP/1.1

### Http/2

 - [rfc9113](https://datatracker.ietf.org/doc/html/rfc9113)
    : HTTP/2

### Http/3

 - [rfc9114](https://datatracker.ietf.org/doc/html/rfc9114)
    : HTTP/3

### Websocket
 - [rfc6455](https://datatracker.ietf.org/doc/html/rfc6455)
    : The WebSocket Protocol
 - [rfc8441](https://datatracker.ietf.org/doc/html/rfc8441)
    : Bootstrapping WebSockets with HTTP/2
 - [rfc9220](https://datatracker.ietf.org/doc/html/rfc9220)
    : Bootstrapping WebSockets with HTTP/3
 - [nginx-websocket-proxying](https://nginx.org/en/docs/http/websocket.html)
    : WebSocket proxying
 - [iana-websocket](https://www.iana.org/assignments/websocket/websocket.xml)
    : WebSocket Protocol Registries

### FTP

 - [rfc959](https://datatracker.ietf.org/doc/html/rfc959)
    : FILE TRANSFER PROTOCOL (FTP)
 - [rfc1639](https://datatracker.ietf.org/doc/html/rfc1639)
    : FTP Operation Over Big Address Records (FOOBAR)
 - [rfc2389](https://datatracker.ietf.org/doc/html/rfc2389)
    : Feature negotiation mechanism for the File Transfer Protocol
 - [rfc2428](https://datatracker.ietf.org/doc/html/rfc2428)
    : FTP Extensions for IPv6 and NATs
 - [rfc2640](https://datatracker.ietf.org/doc/html/rfc2640)
    : Internationalization of the File Transfer Protocol
 - [rfc3659](https://datatracker.ietf.org/doc/html/rfc3659)
    : Extensions to FTP
 - [rfc7151](https://datatracker.ietf.org/doc/html/rfc7151)
    : File Transfer Protocol HOST Command for Virtual Hosts
 - [iana-ftp-commands-extensions](https://www.iana.org/assignments/ftp-commands-extensions/ftp-commands-extensions.xhtml)
    : FTP Commands and Extensions
 - [draft-ietf-ftpext-utf-8-option-00](https://datatracker.ietf.org/doc/html/draft-ietf-ftpext-utf-8-option-00)
    : UTF-8 Option for FTP
 - [draft-ietf-ftpext-data-connection-assurance](https://datatracker.ietf.org/doc/html/draft-ietf-ftpext-data-connection-assurance) 
    : FTP Data Connection Assurance
 - [draft-dd-pret-00](https://datatracker.ietf.org/doc/html/draft-dd-pret-00)
    : Distributed Transfer Support for FTP
 - [draft-rosenau-ftp-single-port-05](https://datatracker.ietf.org/doc/html/draft-rosenau-ftp-single-port-05)
    : FTP EXTENSION ALLOWING IP FORWARDING (NATs)

### SMTP

 - [rfc5321](https://datatracker.ietf.org/doc/html/rfc5321)
    : Simple Mail Transfer Protocol

### POP3

 - [rfc1939](https://datatracker.ietf.org/doc/html/rfc1939)
    : Post Office Protocol - Version 3

### IMAP

 - [rfc3501](https://datatracker.ietf.org/doc/html/rfc3501)
    : INTERNET MESSAGE ACCESS PROTOCOL - VERSION 4rev1
 - [rfc7162](https://datatracker.ietf.org/doc/html/rfc7162)
    : IMAP Extensions: Quick Flag Changes Resynchronization (CONDSTORE) and Quick Mailbox Resynchronization (QRESYNC)

### NNTP

 - [rfc3977](https://datatracker.ietf.org/doc/html/rfc3977)
    : Network News Transfer Protocol (NNTP)
 - [rfc8143](https://datatracker.ietf.org/doc/html/rfc8143)
    : Using Transport Layer Security (TLS) with Network News Transfer Protocol (NNTP)

### MQTT

 - [mqtt-v5.0-os](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html)
    : MQTT Version 5.0 OASIS Standard
 - [mqtt-v3.1.1-os](http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html)
    : MQTT Version 3.1.1 OASIS Standard

### STOMP

 - [stomp-specification-1.2](https://stomp.github.io/stomp-specification-1.2.html)
    : https://stomp.github.io/stomp-specification-1.2.html

### RTMP

 - [rtmp_specification_1.0](https://rtmp.veriskope.com/docs/spec/)
    : Adobe RTMP Specification

### RTSP/2.0

 - [rfc7826](https://datatracker.ietf.org/doc/html/rfc7826)
    : Real-Time Streaming Protocol Version 2.0

### BitTorrent

 - [bep_0003](http://bittorrent.org/beps/bep_0003.html)
    : The BitTorrent Protocol Specification

### ICAP

 - [rfc3507](https://datatracker.ietf.org/doc/html/rfc3507)
    : Internet Content Adaptation Protocol (ICAP)
 - [draft-icap-ext-partial-content-07](http://www.icap-forum.org/documents/specification/draft-icap-ext-partial-content-07.txt)
    : ICAP Partial Content Extension

### WCCP

 - [draft-wilson-wrec-wccp-v2-01](https://datatracker.ietf.org/doc/html/draft-wilson-wrec-wccp-v2-01)
    : Web Cache Communication Protocol V2.0

### NAT Traversal

 - [rfc8489](https://datatracker.ietf.org/doc/html/rfc8489)
   : Session Traversal Utilities for NAT (STUN)
 - [rfc8656](https://datatracker.ietf.org/doc/html/rfc8656)
   : Traversal Using Relays around NAT (TURN): Relay Extensions to Session Traversal Utilities for NAT (STUN)
 - [rfc8445](https://datatracker.ietf.org/doc/html/rfc8445)
   : Interactive Connectivity Establishment (ICE): A Protocol for Network Address Translator (NAT) Traversal
