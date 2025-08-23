.. _configuration_server:

******
server
******

The following keys are supported in a single keyless server:

name
----

**required**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

Set the name of the server.

shared_logger
-------------

**optional**, **type**: ascii

Set the server to use a logger running on a shared thread.

**default**: not set

extra_metrics_tags
------------------

**optional**, **type**: :ref:`static metrics tags <conf_value_static_metrics_tags>`

Set extra metrics tags that should be added to server stats and user stats already with server tags added.

**default**: not set

listen
------

**required**, **type**: :ref:`tcp listen <conf_value_tcp_listen>`

Set the listen config for this server.

tls_server
----------

**optional**, **type**: :ref:`openssl server config <conf_value_openssl_server_config>`

Enable TLS on the listening socket and set TLS parameters.

**default**: disabled

multiplex_queue_depth
---------------------

**optional**, **type**: usize

Enable multiplex support and set the queue length.

It is required if you want to use multiple worker backends.

**default**: not set

request_read_timeout
--------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

**default**: 100ms

duration_stats
--------------

**optional**, **type**: :ref:`histogram metrics <conf_value_histogram_metrics>`

Histogram metrics config for time duration stats of request operations.

**default**: set with default value

async_op_timeout
----------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set timeout value for the async operation of a single request.

**default**: 1s

concurrency_limit
-----------------

**optional**, **type**: usize

Set request concurrency limit. Extra requests will be pending in the queue.

**default**: not limited
