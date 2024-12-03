.. _log_task_tcp_connect:

***********
Tcp Connect
***********

The following keys are available for TcpConnect task log:

server_addr
-----------

**required**, **type**: socket address string

The listening address of the server.

client_addr
-----------

**required**, **type**: socket address string

The client address.

upstream
--------

**required**, **type**: domain:port | socket address string

The target upstream that the client want to access.

next_bind_ip
------------

**optional**, **type**: ip address string

The selected bind IP before we really connect to the remote peer.

Present only if bind ip config is enabled on the corresponding escaper.

next_bound_addr
---------------

**optional**, **type**: socket address string

The local address for the remote connection.

Present only if we have connected to the remote peer.

next_peer_addr
--------------

**optional**, **type**: socket address string

The peer address for the remote connection.

The peer may be the upstream, or will be a next proxy address, which depends on the type of escaper.

Present only if we have selected the ip address of the next peer.

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

c_rd_bytes
----------

**optional**, **type**: int

How many bytes we have received from client.

c_wr_bytes
----------

**optional**, **type**: int

How many bytes we have sent to client.

r_rd_bytes
----------

**optional**, **type**: int

How many bytes we have received from the remote peer.

r_wr_bytes
----------

**optional**, **type**: int

How many bytes we have sent to the remote peer.
