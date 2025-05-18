.. _configuration_collector_aggregate:

aggregate
=========

A collector to aggregate metrics.

The following common keys are supported:

* :ref:`next <conf_collector_common_next>`
* :ref:`exporter <conf_collector_common_exporter>`

emit_interval
-------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the time interval to emit internal metrics.

**default**: 1s

join_tags
---------

**optional**, **type**: :ref:`metric tag name <conf_value_metric_tag_name>` | seq

Set the tag(s) used to join metrics after aggregated together.
