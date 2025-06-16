.. _configure_metrics_value_types:

*******
Metrics
*******

.. _conf_value_metric_value:

metric value
============

**yaml value**: limited str

Only the following characters are allowed:

a to z, A to Z, 0 to 9, -, _, ., / or Unicode letters (as per the specification)

The character range is the same as `OpenTSDB metrics-and-tags`_.

.. _OpenTSDB metrics-and-tags: http://opentsdb.net/docs/build/html/user_guide/writing/index.html#metrics-and-tags

.. _conf_value_metric_tag_name:

metric tag name
===============

**yaml value**: :ref:`metric value <conf_value_metric_value>`

Set a metric tag name, which should not be empty.

.. _conf_value_metric_tag_value:

metric tag value
================

**yaml value**: :ref:`metric value <conf_value_metric_value>`

Set a metric tag value, which may be empty according to the context.

.. _conf_value_static_metrics_tags:

static metrics tags
===================

**yaml value**: map

The key should be :ref:`metric tag name <conf_value_metric_tag_name>`.
The value should be :ref:`metric tag value <conf_value_metric_tag_value>`.

.. _conf_value_metric_node_name:

metric node name
================

**yaml value**: :ref:`metrics value <conf_value_metric_value>`

The metrics name

.. _conf_value_weighted_metric_node_name:

weighted metric node name
=========================

**yaml value**: map | :ref:`metric node name <conf_value_metric_node_name>`

A metrics name with weight set, which make can be grouped into selective vector.

The map consists 2 fields:

* name

  **required**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

  The name. The meaning of the name is depending on the config context.

* weight

  **optional**, **type**: f64

  The weight of the name.
  It may be converted to the smallest u32 greater than or equal to the f64 value when used.

  **default**: 1.0

If the value type is string, then it's value will be the *name* field, with *weight* set to default value.

.. _conf_value_metrics_quantile:

metrics quantile
================

**yaml value**: str | float

A quantile value, should be in range 0.0 - 1.0.

It's string value will be used as the value of quantile tag. You should prefer to use str form if you want the tag value
to be the same as you typed in the config file.

.. _conf_value_histogram_metrics:

histogram metrics
=================

**yaml value**: map | :ref:`rotate <conf_value_histogram_metrics_rotate>`

Config histogram metrics, such as the quantiles and rotate interval.

The keys are:

quantile
--------

**optional**, **type**: seq

Set quantile list.

Should be a sequence of :ref:`metrics quantile <conf_value_metrics_quantile>` or a string of them delimited by ','.

**default**: 0.50, 0.80, 0.90, 0.95, 0.99

.. _conf_value_histogram_metrics_rotate:

rotate
------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the rotate interval.

**default**: 4s

.. _conf_value_statsd_client_config:

Statsd Client Config
====================

The full format of the root value should be a map, with the following keys:

target_unix
-----------

**optional**, **type**: mix

You can set this if you want to send statsd metrics to a custom unix socket path.

The value can be a map, with the following keys:

* path

  **required**, **type**: :ref:`absolute path <conf_value_absolute_path>`

  The syslogd daemon listen socket path.

If the value type is str, the value should be the same as the value as *path* above.

**default**: not set

target_udp
----------

**optional**, **type**: mix

You can set this if you want to send statsd metrics to a remote statsd which listening on a udp socket.

The value can be a map, with the following keys:

* address

  **optional**, **type**: :ref:`env sockaddr str <conf_value_env_sockaddr_str>`

  Set the remote socket address.

  **default**: 127.0.0.1:8125

* bind_ip

  **optional**, **type**: :ref:`ip addr str <conf_value_ip_addr_str>`

  Set the ip address to bind to for the local socket.

  **default**: not set

If the value type is str, the value should be the same as the value as *address* above.

target
------

**optional**, **type**: map

This is just another form to set statsd target address.

The key *udp* is just handled as *target_udp* as above.

The key *unix* is just handled as *target_unix* as above.

prefix
------

**optional**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

Set the global prefix for all metrics.

**default**: "g3proxy"

emit_interval
-------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the emit interval for local stats. All stats will be send out in sequence.

**default**: 200ms, **alias**: emit_duration

.. versionchanged:: 1.11.8 name changed to emit_interval
