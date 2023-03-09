.. _metrics_user_site:

#################
User Site Metrics
#################

The metrics in user site side shows the application layer stats for each explicit user sites.

The metrics path will have prefix user.site.<site_id>,
where *site_id* is specified in config option :ref:`id <conf_user_group_user_site_id>`.

The following are the tags for all user metrics:

* :ref:`daemon_group <metrics_tag_daemon_group>`
* :ref:`stat_id <metrics_tag_stat_id>`

* user_group

  Show the name of the user group.

* user

  Show the name of the user.

* user_type

  Show the type of the user. See :ref:`user type <metrics_user_user_type>` for more details.

  .. versionadded:: 1.7.0

Request
=======

The following tags are set for metrics in this section:

* server

  Set the server name that received the request.

Extra tags set at server side will also be added.

The following tag is also set for *user.connection.\** metrics:

* :ref:`connection <metrics_tag_connection>`

The following tag is also set for *user.request.\** metrics:

* :ref:`request <metrics_tag_request>`

The metric names are:

* user.<site_id>.connection.total

  **type**: count

  Show how many client connections from the user. Connections that failed at authentication stage is not counted in.

* user.<site_id>.request.total

  **type**: count

  Show the total requests that has been received from the user. The value should be larger than or equal to the value
  of user.connection.total, as the connection may be reused for some protocols.

* user.<site_id>.request.alive

  **type**: gauge

  Show the alive requests for the user.

* user.<site_id>.request.ready

  **type**: count

  Show the total tasks that have reached the *ready* stage for the user. The remote connection may be a new connection,
  or an old keepalive connection.

* user.<site_id>.request.reuse

  **type**: count

  Show the total number of reuse of the old remote keepalive connections.
  Note the reuse may be failed.

* user.<site_id>.request.renew

  **type**: count

  Show the total number of failed reuse of the old remote keepalive connections. After the old connection failed at some
  recoverable stage, a new connection is made to retry the request.

* user.<site_id>.l7.connection.alive

  **type**: gauge

  Show the alive layer 7 proxy connections.

  .. versionadded:: 1.4.0

Traffic
=======

The following tags are set for metrics in this section:

* :ref:`request <metrics_tag_request>`

* server

  Set the server name that received the request.

Extra tags set at server side will also be added.

The io stats for user only include application layer stats, i.e. the negotiation data in socks protocol is not counted
in, and the tls layer for https forward is not counted in also.

The metric names are:

* user.<site_id>.traffic.in.bytes

  **type**: count

  Show the total bytes received from client.

* user.<site_id>.traffic.in.packets

  **type**: count

  Show the total datagram packets received from client.
  Note that this is not available for stream type transport protocols.

* user.<site_id>.traffic.out.bytes

  **type**: count

  Show the total bytes sent to client.

* user.<site_id>.traffic.out.packets

  **type**: count

  Show the total datagram packets sent to client.
  Note that this is not available for stream type transport protocols.

Upstream Traffic
================

The following tags are set for metrics in this section:

* :ref:`transport <metrics_tag_transport>`

* escaper

  Set the server name that received the request.

Extra tags set at escaper side will also be added.

The io stats for user only include application layer stats, and the tls layer for https forward is not counted in also.

The metric names are:

* user.<site_id>.upstream.traffic.in.bytes

  **type**: count

  Show the total bytes received from upstream.

* user.<site_id>.upstream.traffic.in.packets

  **type**: count

  Show the total datagram packets received from upstream.
  Note that this is not available for stream type transport protocols.

* user.<site_id>.upstream.traffic.out.bytes

  **type**: count

  Show the total bytes sent to upstream.

* user.<site_id>.upstream.traffic.out.packets

  **type**: count

  Show the total datagram packets sent to upstream.
  Note that this is not available for stream type transport protocols.

