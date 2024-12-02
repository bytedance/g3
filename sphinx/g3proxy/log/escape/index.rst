.. _log_escape:

**********
Escape Log
**********

The escape log contains only errors when we need to connect to or send data to remote peer.

Shared Keys
===========

The following shared keys are set in all type of escape logs:

escaper_type
------------

**required**, **type**: enum string

The type of the escaper.

escaper_name
------------

**required**, **type**: string

The name of the escaper.

escape_type
-----------

**required**, **type**: enum string

The subtype of this escape log. The meaning of non-shared keys are depend on this value.

task_id
-------

**required**, **type**: uuid in simple string format

UUID of the task.

The task_id is also contained in task logs.

upstream
--------

**required**, **type**: domain:port | socket address string

The target upstream that the client want to access.

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

Sub Types
=========

.. toctree::
   :maxdepth: 2

   tcp_connect
   tls_handshake
   udp_sendto
