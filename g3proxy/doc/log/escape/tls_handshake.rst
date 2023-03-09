.. _log_escape_tls_handshake:

************
TlsHandshake
************

The following keys are available for TlsHandshake escape log:

next_bind_ip
------------

**optional**, **type**: ip address string

The selected bind IP before we really connect to the remote peer.

Present only if bind ip config is enabled on the corresponding escaper.

next_expire
-----------

**optional**, **type**: rfc3339 timestamp string with microseconds

The expected expire time of the next peer.

Present only if the next escaper is dynamic and we have selected the remote peer.

tls_name
--------

**required**, **type**: domain name or ip string

The name we used to identify the cert of the remote peer.

tls_peer
--------

**required**, **type**: domain:port | socket address string

The remote peer we need to setup TLS with.

tls_application
---------------

**required**, **type**: enum string

Show the application protocol we want to use inside the TLS channel.

The values are:

* HttpForward

  The user send a HttpsForward request, and we need to do setup TLS channel.

* HttpProxy

  The next peer is a https proxy.
