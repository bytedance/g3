.. _metrics_escaper:

###############
Escaper Metrics
###############

The metrics in escaper side shows the stats with remote.

For **non-route escapers**, the *request* and *traffic* metrics are available.
For **route escapers**, the *route* metrics is available.

The following are the tags for all escaper metrics:

* :ref:`daemon_group <metrics_tag_daemon_group>`
* :ref:`stat_id <metrics_tag_stat_id>`

* escaper

  Set the escaper name.

Request
=======

No extra tags. Extra tags set at escaper side will be added.

The metric names are:

* escaper.task.total

  **type**: count

  Show the total tasks that use this escaper.

* escaper.connection.attempt

  **type**: count

  Show the count of connection attempt to remote.

* escaper.connection.establish

  **type**: count

  Show the count of established connections to remote.

* escaper.tcp.connect.attempt

  **type**: count

  Show the count of attempt to TCP connect to the next peer.

  .. versionadded:: 1.11.1

* escaper.tcp.connect.establish

  **type**: count

  Show the count of established TCP connections to the next peer that will be used by tasks.

  .. versionadded:: 1.11.1

* escaper.tcp.connect.success

  **type**: count

  Show the count of success TCP connect to the next peer.

  .. note::

    This is different than *escaper.tcp.connect.establish*, as we may try connect may times in HappyEyeballs,
    but only one successful connection will be used by the task.

  .. versionadded:: 1.11.1

* escaper.tcp.connect.error

  **type**: count

  Show the count of failed (error encountered) TCP connect to the next peer.

  .. versionadded:: 1.11.1

* escaper.tcp.connect.timeout

  **type**: count

  Show the count of failed TCP connect to the next peer due to timeout.

  .. versionadded:: 1.11.1

* escaper.tls.handshake.success

  **type**: count

  Show the count of success TLS handshake to the next peer proxy.

  .. versionadded:: 1.11.1

* escaper.tls.handshake.error

  **type**: count

  Show the count of failed (error encountered) TLS handshake to the next peer proxy.

  .. versionadded:: 1.11.1

* escaper.tls.handshake.timeout

  **type**: count

  Show the count of failed TLS handshake to the next peer proxy due to timeout.

  .. versionadded:: 1.11.1

* escaper.tls.peer.closure.orderly

  **type**: count

  Show the count of received TLS warning alerts from peer, which includes close_notify and user_canceled.

  .. note:: You may see user_canceled followed by a close_notify on one connection.

  .. versionadded:: 1.11.4

* escaper.tls.peer.closure.abortive

  **type**: count

  Show the count of received TLS error alerts (abortive closure of connection) from peer.

  .. versionadded:: 1.11.4

* escaper.forbidden.ip_blocked

  **type**: count

  Show the count of ip blocked connection attempts.

  This stats is also added to user forbidden stats when possible.

Traffic
=======

The following tags are also set:

* :ref:`transport <metrics_tag_transport>`

Extra tags set at escaper side will be added.

The io stats here include stats of the upper layer of transport layer, which means TLS data are also counted in.

The metric names are:

* escaper.traffic.in.bytes

  **type**: count

  Show the total bytes that are received from remote side on this escaper.

* escaper.traffic.in.packets

  **type**: count

  Show the total datagram packets that are received from remote side on this escaper.
  Note that this is not available for stream type transport protocols.

* escaper.traffic.out.bytes

  **type**: count

  Show the total bytes that are sent to remote from this escaper.

* escaper.traffic.out.packets

  **type**: count

  Show the total datagram packets that are sent to remote from this escaper.
  Note that this is not available for stream type transport protocols.

Route
=====

No extra tags.

The metric names are:

* route.request.passed

  **type**: count

  Show how many requests have been successfully routed.

* route.request.failed

  **type**: count

  Show how many requests have been failed at route selection.
