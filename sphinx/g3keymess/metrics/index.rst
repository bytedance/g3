.. _metrics:

#######
Metrics
#######

Currently we only support send metrics to statsd, see :ref:`stat <configuration_stat>` for more details.

Common Tags
===========

The following is the common tags for all metrics:

.. _metrics_tag_daemon_group:

* daemon_group

  This tag is the same as the daemon group specified in config file or command args.

.. _metrics_tag_stat_id:

* stat_id

  A machine local unique stat_id for dedup purpose. It should be **dropped** by statsd, and the metrics with the same
  remaining tags should be aggregated.

.. _metrics_tag_quantile:

* quantile

  Show the quantile value for histogram stats.

  The following values are always persent:

  - min
  - max
  - mean

  Values can be added by :ref:`histogram metrics <conf_value_histogram_metrics>` config.
  If not set, the following values are added by default:

  - 0.50
  - 0.80
  - 0.90
  - 0.95
  - 0.99

Metrics Types
=============

.. toctree::

   server
