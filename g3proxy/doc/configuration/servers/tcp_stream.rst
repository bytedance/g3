.. _configuration_server_tcp_stream:

tcp_stream
==========

A simple tcp stream server. Map local tcp port to remote tcp port.

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

upstream
--------

**required**, **type**: :ref:`upstream str <conf_value_upstream_str>` | seq

Set the remote address(es) and port. The *port* field is always required.

For *seq* value, each of its element must be :ref:`weighted upstream addr <conf_value_weighted_upstream_addr>`.

**alias**: proxy_pass

.. versionchanged:: 1.5.3 Allow set multiple upstream addresses.

upstream_pick_policy
----------------------

**optional**, **type**: :ref:`selective pick policy <conf_value_selective_pick_policy>`

Set the policy to select upstream address.

The key for rendezvous/jump hash is *<client-ip>*.

**default**: random

.. versionadded:: 1.5.3

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

.. note:: IP address is not supported by now

**default**: not set
