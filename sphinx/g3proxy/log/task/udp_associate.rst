.. _log_task_udp_associate:

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

initial_peer
------------

**optional**, **type**: socket address string

The target peer address in the first udp packet.

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
