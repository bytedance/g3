.. _protocol_helper_icap_http:

=============
ICAP for HTTP
=============

g3proxy support to enable ICAP reqmod and respmod services for HTTP 1.x request and response.

The following headers will be added in the ICAP request header:

- X-HTTP-Upgrade

  The Upgrade header in request will be converted to X-HTTP-Upgrade header in ICAP request with the same value.
