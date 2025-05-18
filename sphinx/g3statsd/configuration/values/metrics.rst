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

**yaml value**: :ref:`metric value <conf_value_metric_value>`

A single node in the metric name.

.. _conf_value_metric_name_prefix:

metric name prefix
==================

**yaml value**: seq of :ref:`metric node name <conf_value_metric_node_name>` | str

Set prefix for metric name.

This could be an array of metric node name, or a string value delimited by '.'.
