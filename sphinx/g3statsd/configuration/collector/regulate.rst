.. _configuration_collector_regulate:

regulate
========

A collector to regulate metrics.

The following common keys are supported:

* :ref:`next <conf_collector_common_next>`
* :ref:`exporter <conf_collector_common_exporter>`

prefix
------

**optional**, **type**: :ref:`metric name prefix <conf_value_metric_name_prefix>`

Set the prefix to add to all metric names.

drop_tags
---------

**optional**, **type**: :ref:`metric tag name <conf_value_metric_tag_name>` | seq

Set the tag(s) to drop.
