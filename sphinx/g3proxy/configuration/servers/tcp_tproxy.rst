.. _configuration_server_tcp_tproxy:

tcp_tproxy
==========

.. versionadded:: 1.7.34

A simple tcp tproxy server, which will just forward traffic to the targeted remote address.

See :ref:`transparent proxy <protocol_setup_transparent_proxy>` for how to setup the host firewall / route table.

The following common keys are supported:

* :ref:`escaper <conf_server_common_escaper>`
* :ref:`auditor <conf_server_common_auditor>`
* :ref:`user_group <conf_server_common_user_group>`

  The user group should be `facts` authenticate type.
  It will be used only if either `auth_by_client_ip` or `auth_by_server_ip` is set.

  .. versionadded:: 1.13.0

* :ref:`shared_logger <conf_server_common_shared_logger>`
* :ref:`listen_in_worker <conf_server_common_listen_in_worker>`
* :ref:`tcp_sock_speed_limit <conf_server_common_tcp_sock_speed_limit>`
* :ref:`ingress_network_filter <conf_server_common_ingress_network_filter>`
* :ref:`tcp_copy_buffer_size <conf_server_common_tcp_copy_buffer_size>`
* :ref:`tcp_copy_yield_size <conf_server_common_tcp_copy_yield_size>`
* :ref:`tcp_misc_opts <conf_server_common_tcp_misc_opts>`
* :ref:`task_idle_check_interval <conf_server_common_task_idle_check_interval>`
* :ref:`task_idle_max_count <conf_server_common_task_idle_max_count>`
* :ref:`flush_task_log_on_created <conf_server_common_flush_task_log_on_created>`
* :ref:`flush_task_log_on_connected <conf_server_common_flush_task_log_on_connected>`
* :ref:`task_log_flush_interval <conf_server_common_task_log_flush_interval>`
* :ref:`extra_metrics_tags <conf_server_common_extra_metrics_tags>`

listen
------

**required**, **type**: :ref:`tcp listen <conf_value_tcp_listen>`

Set the listen config for this server.

The instance count setting will be ignored if *listen_in_worker* is correctly enabled.

auth_by_client_ip
-----------------

**optional**, **type**: bool, **conflict**: auth_by_server_ip

Enable facts user authenticate and use client IP as the authenticate fact.

**default**: false

.. versionadded:: 1.13.0

auth_by_server_ip
-----------------

**optional**, **type**: bool, **conflict**: auth_by_client_ip

Enable facts user authenticate and use server IP as the authenticate fact.

**default**: false

.. versionadded:: 1.13.0
