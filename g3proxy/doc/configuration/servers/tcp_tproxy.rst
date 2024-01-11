.. _configuration_server_tcp_tproxy:

tcp_tproxy
==========

.. versionadded:: 1.7.34

A simple tcp tproxy server, which will just forward traffic to the targeted remote address.

See :ref:`transparent proxy <protocol_setup_transparent_proxy>` for how to setup the host firewall / route table.

The following common keys are supported:

* :ref:`escaper <conf_server_common_escaper>`
* :ref:`auditor <conf_server_common_auditor>`
* :ref:`shared_logger <conf_server_common_shared_logger>`
* :ref:`listen_in_worker <conf_server_common_listen_in_worker>`
* :ref:`tcp_sock_speed_limit <conf_server_common_tcp_sock_speed_limit>`
* :ref:`ingress_network_filter <conf_server_common_ingress_network_filter>`
* :ref:`tcp_copy_buffer_size <conf_server_common_tcp_copy_buffer_size>`
* :ref:`tcp_copy_yield_size <conf_server_common_tcp_copy_yield_size>`
* :ref:`tcp_misc_opts <conf_server_common_tcp_misc_opts>`
* :ref:`task_idle_check_duration <conf_server_common_task_idle_check_duration>`
* :ref:`task_idle_max_count <conf_server_common_task_idle_max_count>`
* :ref:`extra_metrics_tags <conf_server_common_extra_metrics_tags>`

listen
------

**required**, **type**: :ref:`tcp listen <conf_value_tcp_listen>`

Set the listen config for this server.

The instance count setting will be ignored if *listen_in_worker* is correctly enabled.
