.. _log_escape_tcp_connect:

**********
TcpConnect
**********

The following keys are available for TcpConnect escape log:

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

tcp_connect_tries
-----------------

**optional**, **type**: int

How many times we have tried to connect to the remote peer.

tcp_connect_spend
-----------------

**optional**, **type**: time duration string

How many time we have spent during connection of the remote peer (all tries count in).

reason
------

**required**, **type**: enum string

The brief error reason.
