.. _configuration_server_sni_proxy:

sni_proxy
=========

A tcp forward proxy server based on TLS SNI / HTTP Host.

The following common keys are supported:

* :ref:`escaper <conf_server_common_escaper>`
* :ref:`auditor <conf_server_common_auditor>`
* :ref:`shared_logger <conf_server_common_shared_logger>`
* :ref:`listen <conf_server_common_listen>`
* :ref:`listen_in_worker <conf_server_common_listen_in_worker>`
* :ref:`tcp_sock_speed_limit <conf_server_common_tcp_sock_speed_limit>`
* :ref:`ingress_network_filter <conf_server_common_ingress_network_filter>`
* :ref:`tcp_copy_buffer_size <conf_server_common_tcp_copy_buffer_size>`
* :ref:`tcp_copy_yield_size <conf_server_common_tcp_copy_yield_size>`
* :ref:`tcp_misc_opts <conf_server_common_tcp_misc_opts>`
* :ref:`task_idle_check_duration <conf_server_common_task_idle_check_duration>`
* :ref:`task_idle_max_count <conf_server_common_task_idle_max_count>`
* :ref:`extra_metrics_tags <conf_server_common_extra_metrics_tags>`

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

.. versionadded:: 1.7.0

server_tcp_portmap
------------------

**optional**, **type**: :ref:`server tcp portmap <conf_value_dpi_server_tcp_portmap>`

Set the portmap for protocol inspection based on server side tcp port.

**default**: set with default value

.. versionadded:: 1.7.0

client_tcp_portmap
------------------

**optional**, **type**: :ref:`client tcp portmap <conf_value_dpi_client_tcp_portmap>`

Set the portmap for protocol inspection based on client side tcp port.

**default**: set with default value

.. versionadded:: 1.7.0

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

.. versionadded:: 1.1.1

.. _configuration_server_sni_proxy_host:

Host
^^^^

.. versionadded:: 1.1.1

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
