.. _metrics_server:

##############
Server Metrics
##############

The metrics in server side shows the stats with client, and can be grouped to *request* and *traffic* types.

The following are the tags for all server metrics:

* :ref:`daemon_group <metrics_tag_daemon_group>`
* :ref:`stat_id <metrics_tag_stat_id>`

* server

  Show the server name.

* online

  Show if the server is online. The value is either 'y' or 'n'.

Listen
======

No extra tags.

The metric names are:

* listen.instance.count

  **type**: gauge

  Show how many listening sockets.

* listen.accepted

  **type**: count

  Show how many client connections has been accepted.

* listen.dropped

  **type**: count

  Show how many client connections has been dropped by acl rules at early stage.

* listen.timeout

  **type**: count

  Show how many client connections has been timed out in early protocol negotiation (such as TLS).

* listen.failed

  **type**: count

  Show how many times of accept error.

Request
=======

No other fixed tags. Extra tags set at server side will be added.

The metric names are:

* server.connection.total

  **type**: count

  Show how many client connections has been accepted.

* server.task.total

  **type**: count

  Show how many valid tasks has been spawned. Each client connection will be promoted to task only if the negotiation
  success. User authentication is also taken into count in negotiation stage.

* server.task.alive

  **type**: gauge

  Show how many alive tasks that spawned by this server are running. In normal case the daemon stopped by systemd,
  servers with running tasks will goto offline mode, and wait all tasks to be stopped.

Forbidden
=========

No other fixed tags. Extra tags set at server side will be added.

The metric names are:

* server.forbidden.auth_failed

  **type**: count

  Show how many of requests has been forbidden because of auth failed (no user supplied or user token mismatch).

* server.forbidden.dest_denied

  **type**: count

  Show how many of requests has been forbidden because of dest denied (the target upstream address is denied).

  This stats is also added to user forbidden stats when possible.

  .. note:: Only the rules at server level will be counted in.

* server.forbidden.user_blocked

  **type**: count

  Show how many of requests from blocked user.

Traffic
=======

The following tags are also set:

* :ref:`transport <metrics_tag_transport>`

Extra tags set at server side will be added.

The io stats here only include application layer stats, the other layer such TLS stats are not included.

The metric names are:

* server.traffic.in.bytes

  **type**: count

  Show the total bytes of incoming bytes from client.

* server.traffic.in.packets

  **type**: count

  Show the total datagram packets received from client.
  Note that this is not available for stream type transport protocols.

* server.traffic.out.bytes

  **type**: count

  Show the total bytes that the server has sent to the client.

* server.traffic.out.packets

  **type**: count

  Show the total datagram packets that the server has sent to the client.
  Note that this is not available for stream type transport protocols.

Untrusted
=========

What is untrusted task? Invalid but we still need to drain it.

No other fixed tags. Extra tags set at server side will be added.

The metric names are:

* server.task.untrusted_total

  **type**: count

  Show how many untrusted tasks has been spawned.

* server.task.untrusted_alive

  **type**: gauge

  Show how many alive untrusted tasks that spawned by this server are running.

* server.traffic.untrusted_in.bytes

  **type**: count

  Show the total bytes of incoming bytes from client in untrusted requests.
