********
Exporter
********

The type for each exporter config is *map*, with two always required keys:

* :ref:`name <conf_exporter_common_name>`, which specify the name of the exporter.
* :ref:`type <conf_exporter_common_type>`, which specify the real type of the exporter, decides how to parse other keys.

There are many types of exporter, each with a section below.

Exporters
=========

.. toctree::
   :maxdepth: 1

   console
   discard
   graphite
   influxdb_v2
   influxdb_v3
   memory
   opentsdb

Common Keys
===========

This section describes the common keys, they may be used by many exporters.

.. _conf_exporter_common_name:

name
----

**required**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

Set the name of the exporter.

.. _conf_exporter_common_type:

type
----

**required**, **type**: str

Set the type of the exporter.

.. _conf_exporter_common_prefix:

prefix
------

**optional**, **type**: :ref:`metric name prefix <conf_value_metric_name_prefix>`

Set the prefix to add to all metric names.

.. _conf_exporter_common_global_tags:

global_tags
-----------

**optional**, **type**: :ref:`static metrics tags <conf_value_static_metrics_tags>`

Set the tags to add to all metrics.

Export Runtimes
===============

Export runtime is the loop runtime to emit metrics at the given `emit_interval`.

.. _configuration_exporter_runtime_stream:

Stream Export Runtime
---------------------

host
^^^^

**required**, **type**: :ref:`host <conf_value_host>`

Set the peer host name.

port
^^^^

**required**, **type**: u16

Set the port of the peer server.

**default**: each exporter will set a default port value

resolve_retry_wait
^^^^^^^^^^^^^^^^^^

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set how many time to wait before next connect after resolve error.

**default**: 30s

connect_retry_wait
^^^^^^^^^^^^^^^^^^

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set how many time to wait before next connect after connect error.

**default**: 10s

.. _configuration_exporter_runtime_http:

HTTP Export Runtime
-------------------

host
^^^^

**required**, **type**: :ref:`host <conf_value_host>`

Set the peer host name.

port
^^^^

**required**, **type**: u16

Set the port of the peer server.

**default**: each exporter will set a default port value

resolve_retry_wait
^^^^^^^^^^^^^^^^^^

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set how many time to wait before next connect after resolve error.

**default**: 30s

connect_retry_wait
^^^^^^^^^^^^^^^^^^

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set how many time to wait before next connect after connect error.

**default**: 10s

rsp_header_max_size
^^^^^^^^^^^^^^^^^^^

**optional**, **type**: usize

Set the max response header size.

**default**: 8192

body_line_max_length
^^^^^^^^^^^^^^^^^^^^

**optional**, **type**: usize

Set the max line size in the response body.

**default**: 512
