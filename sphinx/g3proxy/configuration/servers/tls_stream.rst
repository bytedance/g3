.. _configuration_server_tls_stream:

tls_stream
==========

A simple tls stream server. Add tls layer to remote tcp port.

The following common keys are supported:

* :ref:`escaper <conf_server_common_escaper>`
* :ref:`auditor <conf_server_common_auditor>`
* :ref:`shared_logger <conf_server_common_shared_logger>`
* :ref:`listen_in_worker <conf_server_common_listen_in_worker>`
* :ref:`tls_server <conf_server_common_tls_server>`

  This is **required**.

* :ref:`tls ticketer <conf_server_common_tls_ticketer>`
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

**optional**, **type**: :ref:`tcp listen <conf_value_tcp_listen>`

Set the listen config for this server.

The instance count setting will be ignored if *listen_in_worker* is correctly enabled.

**default**: not set

.. versionadded:: 1.7.20 change listen config to be optional

upstream
--------

**required**, **type**: :ref:`upstream str <conf_value_upstream_str>` | seq

Set the remote address(es) and port. The *port* field is always required.

For *seq* value, each of its element must be :ref:`weighted upstream addr <conf_value_weighted_upstream_addr>`.

**alias**: proxy_pass

upstream_pick_policy
----------------------

**optional**, **type**: :ref:`selective pick policy <conf_value_selective_pick_policy>`

Set the policy to select upstream address.

The key for ketama/rendezvous/jump hash is *<client-ip><server-ip>*.

**default**: random

tls_client
----------

**optional**, **type**: bool | :ref:`openssl tls client config <conf_value_openssl_tls_client_config>`

Set if we should do tls handshake with upstream.

**default**: disabled

upstream_tls_name
-----------------

**optional**, **type**: :ref:`tls name <conf_value_tls_name>`

Set an explicit tls server name to do upstream tls certificate verification.

If not set, the host of upstream address will be used.

**default**: not set
