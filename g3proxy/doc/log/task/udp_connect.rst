.. _log_task_udp_connect:

*************
Udp Associate
*************

The following keys are available for UdpAssociate task log:

tcp_server_addr
---------------

**required**, **type**: socket address string

The server address for the tcp control connection.

tcp_client_addr
---------------

**required**, **type**: socket address string

The client address for the tcp control connection.

udp_server_addr
---------------

**optional**, **type**: socket address string

The server address for the udp data connection.

udp_client_addr
---------------

**optional**, **type**: socket address string

The client address for the udp data connection.

upstream
--------

**required**, **type**: domain:port | socket address string

The target upstream that the client want to access.

next_bind_ip
------------

**optional**, **type**: ip address string

The selected bind IP before we really setup the remote side udp socket.

Present only if bind ip config is enabled on the corresponding escaper.

next_bound_addr
---------------

**optional**, **type**: socket address string

The local address for the remote udp socket.

next_peer_addr
--------------

**optional**, **type**: socket address string

The peer address for the remote udp socket.

The peer may be the upstream, or will be a next proxy address, which depends on the type of escaper.

next_expire
-----------

**optional**, **type**: rfc3339 timestamp string with microseconds

The expected expire time of the next peer.

Present only if the next escaper is dynamic and we have selected the remote peer.

c_rd_bytes
----------

**optional**, **type**: int

How many bytes we have received from client.

c_rd_packets
------------

**optional**, **type**: int

How many packets we have received from client.

c_wr_bytes
----------

**optional**, **type**: int

How many bytes we have sent to client.

c_wr_packets
------------

**optional**, **type**: int

How many packets we have sent to client.

r_rd_bytes
----------

**optional**, **type**: int

How many bytes we have received from the remote peer.

r_rd_packets
------------

**optional**, **type**: int

How many packets we have received from the remote peer.

r_wr_bytes
----------

**optional**, **type**: int

How many bytes we have sent to the remote peer.

r_wr_packets
------------

**optional**, **type**: int

How many packets we have sent to the remote peer.
