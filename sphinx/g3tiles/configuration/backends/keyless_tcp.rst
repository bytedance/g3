.. _configuration_backend_keyless_tcp:

***********
keyless_tcp
***********

A keyless tcp/tls connect backend.

This will only work with keyless tasks.

Config Keys
===========

The following common keys are supported:

* :ref:`discover <conf_backend_common_discover>`
* :ref:`discover_data <conf_backend_common_discover_data>`
* :ref:`extra_metrics_tags <conf_backend_common_extra_metrics_tags>`

tls_client
----------

**optional**, **type**: :ref:`rustls client config <conf_value_rustls_client_config>`

Enable TLS and set TLS parameters for this local TLS client.

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

response_recv_timeout
---------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the timeout value for the waiting of the response.

If timeout, the request will be dropped for the local buffer and an internal error response will be send to client.

**default**: 4s

connection_max_request_count
----------------------------

**optional**, **type**: usize

Set the max number of requests that can ben handled by a single upstream connection.

**default**: 4000

.. versionadded:: 0.3.4

connection_alive_time
---------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the max alive time for a single upstream connection.

**default**: 1h

.. versionadded:: 0.3.4

graceful_close_wait
-------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the graceful wait time duration before we close an alive connection.

**default**: 10s

connection_pool
---------------

**optional**, **type**: :ref:`connection pool <conf_value_connection_pool_config>`

Set the connection pool config.

**default**: set with max idle 8192 min idle 256

.. versionadded:: 0.3.5

tcp_keepalive
-------------

**optional**, **type**: :ref:`tcp keepalive <conf_value_tcp_keepalive>`

Set tcp keepalive.

**default**: no keepalive set

wait_new_channel
----------------

**optional**, **type**: bool

Set if we should wait for new connections when no alive connections available.

**default**: false

.. versionadded:: 0.3.5
