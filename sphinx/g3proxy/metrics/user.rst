.. _metrics_user:

############
User Metrics
############

The metrics in user side shows the application layer stats for users,
and can be grouped to *request* and *traffic* types.

The following are the tags for all user metrics:

* :ref:`daemon_group <metrics_tag_daemon_group>`
* :ref:`stat_id <metrics_tag_stat_id>`

* user_group

  Show the name of the user group.

* user

  Show the name of the user.

.. _metrics_user_user_type:

* user_type

  Show the type of the user.

  Current supported values are:

    - Static
    - Dynamic

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

* user.connection.total

  **type**: count

  Show how many client connections from the user. Connections that failed at authentication stage is not counted in.

* user.forbidden.auth_failed

  **type**: count

  Show how many auth failed forbidden requests (user token mismatch).

* user.forbidden.user_expired

  **type**: count

  Show how many user expired forbidden requests (user has been expired while handling the request).

* user.forbidden.user_blocked

  **type**: count

  Show how many user blocked forbidden requests (user has been blocked while handling the request).

* user.forbidden.fully_loaded

  **type**: count

  Show how many requests has been dropped as the max alive requests limit has reached.

* user.forbidden.rate_limited

  **type**: count

  Show how many rate limited forbidden requests (user request limit quota reached).

* user.forbidden.proto_banned

  **type**: count

  Show how many protocol banned forbidden requests (proxy request type banned).

* user.forbidden.dest_denied

  **type**: count

  Show how many dest denied forbidden requests (the target upstream address is forbidden).

  Those limited by server level rules are also counted in.

* user.forbidden.ip_blocked

  **type**: count

  Show how many ip blocked forbidden requests (the resolved ip address is blocked).

  Those limited by escaper level rules are also counted in.

* user.forbidden.log_skipped

  **type**: count

  Show how many requests has been log skipped (just skipped logging).

* user.forbidden.ua_blocked

  **type**: count

  Show how many layer-7 http requests has been blocked by User-Agent match.

* user.request.total

  **type**: count

  Show the total requests that has been received from the user. The value should be larger than or equal to the value
  of user.connection.total, as the connection may be reused for some protocols.

* user.request.alive

  **type**: gauge

  Show the alive requests for the user.

* user.request.ready

  **type**: count

  Show the total tasks that have reached the *ready* stage for the user. The remote connection may be a new connection,
  or an old keepalive connection.

* user.request.reuse

  **type**: count

  Show the total number of reuse of the old remote keepalive connections.
  Note the reuse may be failed.

* user.request.renew

  **type**: count

  Show the total number of failed reuse of the old remote keepalive connections. After the old connection failed at some
  recoverable stage, a new connection is made to retry the request.

* user.l7.connection.alive

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

* user.traffic.in.bytes

  **type**: count

  Show the total bytes received from client.

* user.traffic.in.packets

  **type**: count

  Show the total datagram packets received from client.
  Note that this is not available for stream type transport protocols.

* user.traffic.out.bytes

  **type**: count

  Show the total bytes sent to client.

* user.traffic.out.packets

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

* user.upstream.traffic.in.bytes

  **type**: count

  Show the total bytes received from upstream.

* user.upstream.traffic.in.packets

  **type**: count

  Show the total datagram packets received from upstream.
  Note that this is not available for stream type transport protocols.

* user.upstream.traffic.out.bytes

  **type**: count

  Show the total bytes sent to upstream.

* user.upstream.traffic.out.packets

  **type**: count

  Show the total datagram packets sent to upstream.
  Note that this is not available for stream type transport protocols.

