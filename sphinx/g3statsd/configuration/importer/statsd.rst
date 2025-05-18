.. _configuration_importer_statsd:

statsd
======

StatsD importer.

The following common keys are supported:

* :ref:`collector <conf_importer_common_collector>`
* :ref:`listen_in_worker <conf_importer_common_listen_in_worker>`
* :ref:`ingress_network_filter <conf_importer_common_ingress_network_filter>`

listen
------

**optional**, **type**: :ref:`udp listen <conf_value_udp_listen>`

Set the listen config for this importer.

The instance count setting will be ignored if *listen_in_worker* is correctly enabled.

**default**: not set
