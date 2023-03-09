.. _protocol_client:

###############
Client Protocol
###############

We support the following protocols for clients:

* http proxy

  - Both HttpForward and HttpConnect are supported.
  - HttpsForward is also supported, but not enabled by default.
  - Only http version 1.0 and 1.1. Currently no support for http version 2 and 3.
  - Only Basic auth is supported.
  - TLS 1.2 and above can be enabled.
  - see :doc:`http_custom_headers` for custom headers.
  - see :doc:`http_custom_codes` for custom reply codes.

* socks proxy

  - socks4 and socks4a are supported (no ident verification) with most escapers.
  - socks5 TcpConnect is supported with most escapers.
  - socks5 UdpAssociate is supported with some escapers but disabled by default at server side. The default enabled one
    is UdpConnect which is much simplified, but require the target address for each packet to be the same.
    The address family for the tcp and udp connection at client side should be the same if no explicit bind ip set.
  - socks5 User auth is the only one that we support yet.
  - no TLS and DTLS support yet.
  - no socks6 support yet.
  - see :doc:`socks5_custom_reply` for socks5 custom reply field.

.. toctree::
   :hidden:

   http_custom_headers
   http_custom_codes
   socks5_custom_reply
   egress_path_selection
