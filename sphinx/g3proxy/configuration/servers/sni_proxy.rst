.. _configuration_server_sni_proxy:

sni_proxy
=========

A tcp forward proxy server based on TLS SNI / HTTP Host.

The following common keys are supported:

* :ref:`escaper <conf_server_common_escaper>`
* :ref:`auditor <conf_server_common_auditor>`
* :ref:`user_group <conf_server_common_user_group>`

  The user group should be `facts` authenticate type.
  It will be used only if either `auth_by_client_ip` or `auth_by_server_name` is set.

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

**optional**, **type**: :ref:`tcp listen <conf_value_tcp_listen>`

Set the listen config for this server.

The instance count setting will be ignored if *listen_in_worker* is correctly enabled.

**default**: not set

.. versionadded:: 1.7.20 change listen config to be optional

auth_by_client_ip
-----------------

**optional**, **type**: bool, **conflict**: auth_by_server_ip

Enable facts user authenticate and use client IP as the authenticate fact.

**default**: false

.. versionadded:: 1.13.0

auth_by_server_name
-------------------

**optional**, **type**: bool, **conflict**: auth_by_client_ip

Enable facts user authenticate and use server name as the authenticate fact.

**default**: false

.. versionadded:: 1.13.0

tls_max_client_hello_size
-------------------------

**optional**, **type**: u32

Set the max size limit for TLS client hello message.

**default**: 1 << 16

.. versionadded:: 1.9.9

request_wait_timeout
--------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the timeout value for the wait of initial client data.

**default**: 60s

request_recv_timeout
--------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the timeout value for the receive of the complete initial request after the arriving of initial data,
which may be a TLS ClientHello message or a HTTP Request.

**default**: 4s

protocol_inspection
-------------------

**optional**, **type**: :ref:`protocol inspection <conf_value_dpi_protocol_inspection>`

Set basic config for protocol inspection.

**default**: set with default value

server_tcp_portmap
------------------

**optional**, **type**: :ref:`server tcp portmap <conf_value_dpi_server_tcp_portmap>`

Set the portmap for protocol inspection based on server side tcp port.

**default**: set with default value

client_tcp_portmap
------------------

**optional**, **type**: :ref:`client tcp portmap <conf_value_dpi_client_tcp_portmap>`

Set the portmap for protocol inspection based on client side tcp port.

**default**: set with default value

allowed_hosts
-------------

**optional**, **type**: :ref:`host matched object <conf_value_host_matched_object>` <:ref:`host <configuration_server_sni_proxy_host>`>

Set the list of hosts we should handle based on host match rules.

If not set, all requests will be handled.

Example:

.. code-block:: yaml

  hosts:
    - exact_match:
        - www.example.net
        - example.net
      redirect_host: www.example.net:443 # all redirect to www.example.net:*
    - child_match: example.org # pass all *.example.org:*

**default**: not set

.. _configuration_server_sni_proxy_host:

Host
^^^^

This set the config for a SNI host.

redirect_host
"""""""""""""

**optional**, **type**: :ref:`host <conf_value_host>`

Change the host field of the upstream address.

**default**: not set

redirect_port
"""""""""""""

**optional**, **type**: u16

Change the port field of the upstream address.

**default**: not set
