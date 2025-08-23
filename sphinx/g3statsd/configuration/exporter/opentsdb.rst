.. _configuration_exporter_opentsdb:

opentsdb
========

Emit all metrics from collector to opentsdb by using the json `PUT API`_.

.. _PUT API: https://opentsdb.net/docs/build/html/api_http/put.html

The following common keys are supported:

* :ref:`prefix <conf_exporter_common_prefix>`
* :ref:`global_tags <conf_exporter_common_global_tags>`

The :ref:`HTTP Export Runtime <configuration_exporter_runtime_http>` is used:

- default port 4242
- all config keys supported

emit_interval
-------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the time interval to emit internal metrics.

**default**: 10s

sync_timeout
------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set sync and sync_timeout query parameter.

**default**: not set

max_data_points
---------------

**optional**, **type**: usize

Set the max data points that should be sent in a single HTTP request.

**default**: 50
