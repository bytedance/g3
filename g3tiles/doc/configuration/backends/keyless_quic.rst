.. _configuration_backend_keyless_quic:

************
keyless_quic
************

A keyless quic connect backend.

This will only work with keyless tasks.

Config Keys
===========

The following common keys are supported:

* :ref:`discover <conf_backend_common_discover>`
* :ref:`discover_data <conf_backend_common_discover_data>`
* :ref:`extra_metrics_tags <conf_backend_common_extra_metrics_tags>`

tls_client
----------

**required**, **type**: :ref:`rustls client config <conf_value_rustls_client_config>`

Set TLS parameters for this local QUIC client.

**default**: not set

tls_name
--------

**optional**, **type**: :ref:`tls name <conf_value_tls_name>`

Set the tls server name to verify tls certificate for all peers.

If not set, the peer IP will be used.

**default**: not set

duration_stats
--------------

**optional**, **type**: :ref:`histogram metrics <conf_value_histogram_metrics>`

Histogram metrics config for the tcp connect duration stats.

**default**: set with default value

request_buffer_size
-------------------

**optional**, **type**: usize

Set the request buffer size of the local queue. New connections will be opened when the queue is full.

**default**: 128

response_timeout
----------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the timeout value for the waiting of the response.

If timeout, the request will be dropped for the local buffer and an internal error response will be send to client.

**default**: 4s

graceful_close_wait
-------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the graceful wait time duration before we close an alive connection.

**default**: 10s

idle_connection_min
-------------------

**optional**, **type**: usize

Set the minimum number of IDLE connections in the pool.

**default**: 32

idle_connection_max
-------------------

**optional**, **type**: usize

Set the maximum number of IDLE connections in the pool.

**default**: 1024

connect_check_interval
----------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the time interval for the check of closed connections.

Failed connections won't trigger a re-connect again until this check interval reached.

**default**: 10s

.. versionadded:: 0.3.4

concurrent_streams
------------------

**optional**, **type**: usize

Set how many bidirectional streams we will use on a single QUIC connection.

**default**: 4

socket_buffer
-------------

**optional**, **type**: :ref:`socket buffer config <conf_value_socket_buffer_config>`

Set the buffer config for the udp socket.

**default**: not set
