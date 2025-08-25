.. _configuration_server_keyless_proxy:

keyless_proxy
=============

A keyless reverse proxy server.

The following common keys are supported:

* :ref:`shared_logger <conf_server_common_shared_logger>`
* :ref:`ingress_network_filter <conf_server_common_ingress_network_filter>`
* :ref:`task_idle_check_interval <conf_server_common_task_idle_check_interval>`
* :ref:`task_idle_max_count <conf_server_common_task_idle_max_count>`
* :ref:`extra_metrics_tags <conf_server_common_extra_metrics_tags>`

backend
-------

**required**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

Set the backend name.

spawn_task_unconstrained
------------------------

**optional**, **type**: bool

Set if we should spawn tasks in tokio unconstrained way.

**default**: false
