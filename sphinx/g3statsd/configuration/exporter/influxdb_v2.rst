.. _configuration_exporter_influxdb_v2:

influxdb_v2
===========

Emit all metrics from collector to influxdb by using the `v2 write API`_.

.. _v2 write API: https://docs.influxdata.com/influxdb/v2/write-data/developer-tools/api/

The following common keys are supported:

* :ref:`prefix <conf_exporter_common_prefix>`
* :ref:`global_tags <conf_exporter_common_global_tags>`

The :ref:`HTTP Export Runtime <configuration_exporter_runtime_http>` is used:

- default port 8181
- all config keys supported

emit_interval
-------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the time interval to emit internal metrics.

**default**: 10s

bucket
------

**required**, **type**: :ref:`http header value <conf_value_http_header_value>`

Set the bucket name.

token
-----

**optional**, **type**: :ref:`http header value <conf_value_http_header_value>`

Set the auth token.

If not set, the value in environment variable `INFLUX_TOKEN` will be used.

**default**: not set

precision
---------

**optional**, **type**: string

Set the precision query parameter.

Allowed values are:

- s
- ms
- us
- ns

**default**: s

max_body_lines
--------------

**optional**, **type**: usize

Set the max body lines in a single request.

**default**: 10000
