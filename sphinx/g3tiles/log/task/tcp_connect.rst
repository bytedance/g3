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
