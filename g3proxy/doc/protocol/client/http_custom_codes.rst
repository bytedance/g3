.. _protocol_client_http_custom_codes:

#################
Http Custom Codes
#################

The following custom HTTP reply codes are used:

* 521 WEB_SERVER_IS_DOWN

  The upstream or the next peer has refused or reset our connection request.

* 522 CONNECTION_TIMED_OUT

  Timeout to connect to upstream or next peer.

* 523 ORIGIN_IS_UNREACHABLE

  For network error, such network unreachable and host unreachable, occurred while connecting to upstream or next peer.

* 525 SSL_HANDSHAKE_FAILED

  Tls handshake with upstream failed.

  .. note::

    Tls handshake with next proxy peer (it's not upstream) will generate internal server error instead,
    as we usually use different tls client config for proxy peers.

* 530 ORIGIN_DNS_ERROR

  Failed to resolve the ip address of upstream or next peer.
