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
