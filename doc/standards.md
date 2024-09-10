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

## Systemd Journal

 - [JOURNAL_NATIVE_PROTOCOL](https://systemd.io/JOURNAL_NATIVE_PROTOCOL/)
    : Native Journal Protocol

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

## StatsD

 - [the-dogstatsd-protocol](https://docs.datadoghq.com/developers/dogstatsd/datagram_shell?tab=metrics#the-dogstatsd-protocol)
    : The DogStatsD protocol

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
 - [rfc9295](https://datatracker.ietf.org/doc/html/rfc9295/)
    : Clarifications for Ed25519, Ed448, X25519, and X448 Algorithm Identifiers

## Cryptography

 - [NIST SP 800-186](https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-186.pdf)
    : Recommendations for Discrete Logarithm-based Cryptography: Elliptic Curve Domain Parameters
 - [SEC2-v2](https://www.secg.org/sec2-v2.pdf)
    : SEC 2: Recommended Elliptic Curve Domain Parameters

## Electronic Mail

 - [rfc5322](https://datatracker.ietf.org/doc/html/rfc5322)
    : Internet Message Format
 - [rfc7817](https://datatracker.ietf.org/doc/html/rfc7817)
    : Updated Transport Layer Security (TLS) Server Identity Check Procedure for Email-Related Protocols
 - [rfc8314](https://datatracker.ietf.org/doc/html/rfc8314)
    : Cleartext Considered Obsolete: Use of Transport Layer Security (TLS) for Email Submission and Access
 - [iana-mail-parameters](https://www.iana.org/assignments/mail-parameters/mail-parameters.xhtml)
    : MAIL Parameters

## MIME

 - [rfc2045](https://datatracker.ietf.org/doc/html/rfc2045)
    : Multipurpose Internet Mail Extensions (MIME) Part One: Format of Internet Message Bodies
 - [rfc2046](https://datatracker.ietf.org/doc/html/rfc2046)
    : Multipurpose Internet Mail Extensions (MIME) Part Two: Media Types
 - [rfc2047](https://datatracker.ietf.org/doc/html/rfc2047)
    : MIME (Multipurpose Internet Mail Extensions) Part Three: Message Header Extensions for Non-ASCII Text
 - [rfc2231](https://datatracker.ietf.org/doc/html/rfc2231)
    : MIME Parameter Value and Encoded Word Extensions: Character Sets, Languages, and Continuations

# Network Protocol

## Happy Eyeballs

 - [rfc8305](https://datatracker.ietf.org/doc/html/rfc8305)
    : Happy Eyeballs Version 2: Better Connectivity Using Concurrency

## PROXY protocol

 - [haproxy-proxy-protocol](https://github.com/haproxy/haproxy/blob/master/doc/proxy-protocol.txt)
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

 - [rfc1035](https://datatracker.ietf.org/doc/html/rfc1035)
    :  DOMAIN NAMES - IMPLEMENTATION AND SPECIFICATION
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
 - [rfc8998](https://datatracker.ietf.org/doc/html/rfc8998)
    : ShangMi (SM) Cipher Suites for TLS 1.3
 - [rfc6066](https://datatracker.ietf.org/doc/html/rfc6066)
    : Transport Layer Security (TLS) Extensions: Extension Definitions
 - [rfc9325](https://datatracker.ietf.org/doc/html/rfc9325)
    : Recommendations for Secure Use of Transport Layer Security (TLS) and Datagram Transport Layer Security (DTLS)
 - [iana-tls-extensiontype-values](https://www.iana.org/assignments/tls-extensiontype-values/tls-extensiontype-values.xhtml)
    : Transport Layer Security (TLS) Extensions
 - [GB/T 38636-2020](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=778097598DA2761E94A5FF3F77BD66DA)
    : Information security technology—Transport layer cryptography protocol(TLCP)

## QUIC

 - [rfc9000](https://datatracker.ietf.org/doc/html/rfc9000)
    : QUIC: A UDP-Based Multiplexed and Secure Transport
 - [rfc9001](https://datatracker.ietf.org/doc/html/rfc9001)
    : Using TLS to Secure QUIC
 - [rfc9369](https://datatracker.ietf.org/doc/html/rfc9369)
    : QUIC Version 2

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
 - [rfc8297](https://datatracker.ietf.org/doc/html/rfc8297)
    : An HTTP Status Code for Indicating Hints
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
 - [rfc8941](https://datatracker.ietf.org/doc/html/rfc8941)
    : Structured Field Values for HTTP
 - [rfc9297](https://datatracker.ietf.org/doc/html/rfc9297)
    : HTTP Datagrams and the Capsule Protocol
 - [rfc9298](https://datatracker.ietf.org/doc/html/rfc9298)
    : Proxying UDP in HTTP
 - [draft-ietf-masque-connect-ip](https://datatracker.ietf.org/doc/draft-ietf-masque-connect-ip/13/)
    : Proxying IP in HTTP
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

## FTP

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

## SMTP

 - [rfc5321](https://datatracker.ietf.org/doc/html/rfc5321)
    : Simple Mail Transfer Protocol
 - [rfc6409](https://datatracker.ietf.org/doc/html/rfc6409)
    : Message Submission for Mail
 - [rfc2645](https://datatracker.ietf.org/doc/html/rfc2645)
    : ON-DEMAND MAIL RELAY (ODMR) SMTP with Dynamic IP Addresses
 - [rfc6152](https://datatracker.ietf.org/doc/html/rfc6152)
    : SMTP Service Extension for 8-bit MIME Transport
 - [rfc3030](https://datatracker.ietf.org/doc/html/rfc3030)
    : SMTP Service Extensions for Transmission of Large and Binary MIME Messages
 - [rfc4468](https://datatracker.ietf.org/doc/html/rfc4468)
    : Message Submission BURL Extension
 - [rfc1870](https://datatracker.ietf.org/doc/html/rfc1870)
    : SMTP Service Extension for Message Size Declaration
 - [rfc2852](https://datatracker.ietf.org/doc/html/rfc2852)
    : Deliver By SMTP Service Extension
 - [rfc3207](https://datatracker.ietf.org/doc/html/rfc3207)
    : SMTP Service Extension for Secure SMTP over Transport Layer Security
 - [rfc8689](https://datatracker.ietf.org/doc/html/rfc8689)
    : SMTP Require TLS Option
 - [rfc4954](https://datatracker.ietf.org/doc/html/rfc4954)
    : SMTP Service Extension for Authentication
 - [rfc2034](https://datatracker.ietf.org/doc/html/rfc2034)
    : SMTP Service Extension for Returning Enhanced Error Codes
 - [rfc2920](https://datatracker.ietf.org/doc/html/rfc2920)
    : SMTP Service Extension for Command Pipelining
 - [rfc3461](https://datatracker.ietf.org/doc/html/rfc3461)
    : Simple Mail Transfer Protocol (SMTP) Service Extension for Delivery Status Notifications (DSNs)
 - [rfc1985](https://datatracker.ietf.org/doc/html/rfc1985)
    : SMTP Service Extension for Remote Message Queue Starting
 - [rfc3865](https://datatracker.ietf.org/doc/html/rfc3865)
    : A No Soliciting Simple Mail Transfer Protocol (SMTP) Service Extension
 - [rfc3885](https://datatracker.ietf.org/doc/html/rfc3885)
    : SMTP Service Extension for Message Tracking
 - [rfc4865](https://datatracker.ietf.org/doc/html/rfc4865)
    : SMTP Submission Service Extension for Future Message Release
 - [rfc4141](https://datatracker.ietf.org/doc/html/rfc4141)
    : SMTP and MIME Extensions for Content Conversion
 - [rfc6531](https://datatracker.ietf.org/doc/html/rfc6531)
    : SMTP Extension for Internationalized Email
 - [rfc6710](https://datatracker.ietf.org/doc/html/rfc6710)
    : Simple Mail Transfer Protocol Extension for Message Transfer Priorities
 - [rfc7293](https://datatracker.ietf.org/doc/html/rfc7293)
    : The Require-Recipient-Valid-Since Header Field and SMTP Service Extension
 - [rfc9422](https://datatracker.ietf.org/doc/html/rfc9422)
    : The LIMITS SMTP Service Extension
 - [exim-the_smtp_transport](https://www.exim.org/exim-html-current/doc/html/spec_html/ch-the_smtp_transport.html)
    : The smtp transport

## POP3

 - [rfc1939](https://datatracker.ietf.org/doc/html/rfc1939)
    : Post Office Protocol - Version 3

## IMAP

 - [iana-imap-capabilities](https://www.iana.org/assignments/imap-capabilities/imap-capabilities.xhtml)
    : Internet Message Access Protocol (IMAP) Capabilities Registry
 - [rfc7162](https://datatracker.ietf.org/doc/html/rfc7162)
    : IMAP Extensions: Quick Flag Changes Resynchronization (CONDSTORE) and Quick Mailbox Resynchronization (QRESYNC)
 - [rfc9394](https://datatracker.ietf.org/doc/html/rfc9394)
    : IMAP PARTIAL Extension for Paged SEARCH and FETCH
 - [rfc5267](https://datatracker.ietf.org/doc/html/rfc5267)
    : Contexts for IMAP4
 - [rfc5256](https://datatracker.ietf.org/doc/html/rfc5256)
    : Internet Message Access Protocol - SORT and THREAD Extensions
 - [rfc5255](https://datatracker.ietf.org/doc/html/rfc5255)
    : Internet Message Access Protocol Internationalization
 - [rfc6855](https://datatracker.ietf.org/doc/html/rfc6855)
    : IMAP Support for UTF-8
 - [rfc7377](https://datatracker.ietf.org/doc/html/rfc7377)
    : IMAP4 Multimailbox SEARCH Extension
 - [rfc3502](https://datatracker.ietf.org/doc/html/rfc3502)
    : Internet Message Access Protocol (IMAP) - MULTIAPPEND Extension
 - [rfc3516](https://datatracker.ietf.org/doc/html/rfc3516)
    : IMAP4 Binary Content Extension
 - [rfc9208](https://datatracker.ietf.org/doc/html/rfc9208)
    : IMAP QUOTA Extension
 - [rfc4314](https://datatracker.ietf.org/doc/html/rfc4314)
    : IMAP4 Access Control List (ACL) Extension
 - [rfc7889](https://datatracker.ietf.org/doc/html/rfc7889)
    : The IMAP APPENDLIMIT Extension
 - [rfc4467](https://datatracker.ietf.org/doc/html/rfc4467)
    : Internet Message Access Protocol (IMAP) - URLAUTH Extension
 - [rfc5524](https://datatracker.ietf.org/doc/html/rfc5524)
    : Extended URLFETCH for Binary and Converted Parts
 - [rfc4469](https://datatracker.ietf.org/doc/html/rfc4469)
    : Internet Message Access Protocol (IMAP) CATENATE Extension
 - [rfc5550](https://datatracker.ietf.org/doc/html/rfc5550)
    : The Internet Email to Support Diverse Service Environments (Lemonade) Profile
  - [rfc4978](https://datatracker.ietf.org/doc/html/rfc4978)
    : The IMAP COMPRESS Extension
 - [rfc5259](https://datatracker.ietf.org/doc/html/rfc5259)
    : Internet Message Access Protocol - CONVERT Extension
 - [rfc5466](https://datatracker.ietf.org/doc/html/rfc5466)
    : IMAP4 Extension for Named Searches (Filters)
 - [rfc6785](https://datatracker.ietf.org/doc/html/rfc6785)
    : Support for Internet Message Access Protocol (IMAP) Events in Sieve
 - [rfc9585](https://datatracker.ietf.org/doc/html/rfc9585)
    : IMAP Response Code for Command Progress Notifications
 - [rfc9590](https://datatracker.ietf.org/doc/html/rfc9590)
    : IMAP Extension for Returning Mailbox METADATA in Extended LIST
 - [rfc8440](https://datatracker.ietf.org/doc/html/rfc8440)
    : IMAP4 Extension for Returning MYRIGHTS Information in Extended LIST
 - [rfc2221](https://datatracker.ietf.org/doc/html/rfc2221)
    : IMAP4 Login Referrals
 - [rfc2193](https://datatracker.ietf.org/doc/html/rfc2193)
    : IMAP4 Mailbox Referrals
 - [rfc5464](https://datatracker.ietf.org/doc/html/rfc5464)
    : The IMAP METADATA Extension
 - [rfc5465](https://datatracker.ietf.org/doc/html/rfc5465)
    : The IMAP NOTIFY Extension
 - [rfc8474](https://datatracker.ietf.org/doc/html/rfc8474)
    : IMAP Extension for Object Identifiers
 - [rfc8970](https://datatracker.ietf.org/doc/html/rfc8970)
    : IMAP4 Extension: Message Preview Generation
 - [rfc8508](https://datatracker.ietf.org/doc/html/rfc8508)
    : IMAP REPLACE Extension
 - [rfc8514](https://datatracker.ietf.org/doc/html/rfc8514)
    : Internet Message Access Protocol (IMAP) - SAVEDATE Extension
 - [rfc6203](https://datatracker.ietf.org/doc/html/rfc6203)
    : IMAP4 Extension for Fuzzy Search
 - [rfc5957](https://datatracker.ietf.org/doc/html/rfc5957)
    : Display-Based Address Sorting for the IMAP4 SORT Extension
 - [rfc9586](https://datatracker.ietf.org/doc/html/rfc9586)
    : IMAP Extension for Using and Returning Unique Identifiers (UIDs) Only
 - [rfc8437](https://datatracker.ietf.org/doc/html/rfc8437)
    : IMAP UNAUTHENTICATE Extension for Connection Reuse
 - [rfc5032](https://datatracker.ietf.org/doc/html/rfc5032)
    : WITHIN Search Extension to the IMAP Protocol

## IMAP4rev2

 - [rfc9051](https://datatracker.ietf.org/doc/html/rfc9051)
    : Internet Message Access Protocol (IMAP) - Version 4rev2

## IMAP4rev1
 - [rfc3501](https://datatracker.ietf.org/doc/html/rfc3501)
    : INTERNET MESSAGE ACCESS PROTOCOL - VERSION 4rev1
 - [rfc4315](https://datatracker.ietf.org/doc/html/rfc4315)
    : Internet Message Access Protocol (IMAP) - UIDPLUS extension
 - [rfc4959](https://datatracker.ietf.org/doc/html/rfc4959)
    : IMAP Extension for Simple Authentication and Security Layer (SASL) Initial Client Response
 - [rfc6851](https://datatracker.ietf.org/doc/html/rfc6851)
    : Internet Message Access Protocol (IMAP) - MOVE Extension
 - [rfc2971](https://datatracker.ietf.org/doc/html/rfc2971)
    : IMAP4 ID extension
 - [rfc3691](https://datatracker.ietf.org/doc/html/rfc3691)
    : Internet Message Access Protocol (IMAP) UNSELECT command
 - [rfc3348](https://datatracker.ietf.org/doc/html/rfc3348)
    : The Internet Message Action Protocol (IMAP4) Child Mailbox Extension
 - [rfc2177](https://datatracker.ietf.org/doc/html/rfc2177)
    : IMAP4 IDLE command
 - [rfc2342](https://datatracker.ietf.org/doc/html/rfc2342)
    : IMAP4 Namespace
 - [rfc4731](https://datatracker.ietf.org/doc/html/rfc4731)
    : IMAP4 Extension to SEARCH Command for Controlling What Kind of Information Is Returned
 - [rfc4466](https://datatracker.ietf.org/doc/html/rfc4466)
    : Collected Extensions to IMAP4 ABNF
 - [rfc5182](https://datatracker.ietf.org/doc/html/rfc5182)
    : IMAP Extension for Referencing the Last SEARCH Result
 - [rfc5161](https://datatracker.ietf.org/doc/html/rfc5161)
    : The IMAP ENABLE Extension
 - [rfc5258](https://datatracker.ietf.org/doc/html/rfc5258)
    : Internet Message Access Protocol version 4 - LIST Command Extensions
 - [rfc5819](https://datatracker.ietf.org/doc/html/rfc5219)
   : IMAP4 Extension for Returning STATUS Information in Extended LIST
 - [rfc7888](https://datatracker.ietf.org/doc/html/rfc7888)
    : IMAP4 Non-synchronizing Literals
 - [rfc5530](https://datatracker.ietf.org/doc/html/rfc5530)
    : IMAP Response Codes
 - [rfc6154](https://datatracker.ietf.org/doc/html/rfc6154)
    : IMAP LIST Extension for Special-Use Mailboxes
 - [rfc8438](https://datatracker.ietf.org/doc/html/rfc8438)
    : IMAP Extension for STATUS=SIZE

## NNTP

 - [rfc3977](https://datatracker.ietf.org/doc/html/rfc3977)
    : Network News Transfer Protocol (NNTP)
 - [rfc8143](https://datatracker.ietf.org/doc/html/rfc8143)
    : Using Transport Layer Security (TLS) with Network News Transfer Protocol (NNTP)

## MQTT

 - [mqtt-v5.0-os](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html)
    : MQTT Version 5.0 OASIS Standard
 - [mqtt-v3.1.1-os](http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html)
    : MQTT Version 3.1.1 OASIS Standard

## STOMP

 - [stomp-specification-1.2](https://stomp.github.io/stomp-specification-1.2.html)
    : STOMP Protocol Specification, Version 1.2

## SMPP

 - [SMPP](https://smpp.org/)
    : SMPP Protocol: API to enable SMS messaging between applications and mobiles
 - [SMPP_v5](https://smpp.org/SMPP_v5.pdf)
    : Short Message Peer-to-Peer Protocol Specification Version 5.0

## RTMP

 - [rtmp_specification_1.0](https://rtmp.veriskope.com/docs/spec/)
    : Adobe RTMP Specification

## RTSP/2.0

 - [rfc7826](https://datatracker.ietf.org/doc/html/rfc7826)
    : Real-Time Streaming Protocol Version 2.0

## BitTorrent

 - [bep_0003](http://bittorrent.org/beps/bep_0003.html)
    : The BitTorrent Protocol Specification

## ICAP

 - [rfc3507](https://datatracker.ietf.org/doc/html/rfc3507)
    : Internet Content Adaptation Protocol (ICAP)
 - [draft-icap-ext-partial-content-07](http://www.icap-forum.org/documents/specification/draft-icap-ext-partial-content-07.txt)
    : ICAP Partial Content Extension
 - [draft-stecher-icap-subid-00](https://www.icap-forum.org/documents/specification/draft-stecher-icap-subid-00.txt)
    : ICAP Extensions

## WCCP

 - [draft-wilson-wrec-wccp-v2-01](https://datatracker.ietf.org/doc/html/draft-wilson-wrec-wccp-v2-01)
    : Web Cache Communication Protocol V2.0

## NAT Traversal

 - [rfc8489](https://datatracker.ietf.org/doc/html/rfc8489)
    : Session Traversal Utilities for NAT (STUN)
 - [rfc8656](https://datatracker.ietf.org/doc/html/rfc8656)
    : Traversal Using Relays around NAT (TURN): Relay Extensions to Session Traversal Utilities for NAT (STUN)
 - [rfc8445](https://datatracker.ietf.org/doc/html/rfc8445)
    : Interactive Connectivity Establishment (ICE): A Protocol for Network Address Translator (NAT) Traversal
