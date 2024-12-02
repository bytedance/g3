.. _configuration_server_rustls_proxy:

rustls_proxy
============

A layer-4 TLS reverse proxy server based on OpenSSL or it's variants.

The following common keys are supported:

* :ref:`shared_logger <conf_server_common_shared_logger>`
* :ref:`listen_in_worker <conf_server_common_listen_in_worker>`
* :ref:`tcp_sock_speed_limit <conf_server_common_tcp_sock_speed_limit>`
* :ref:`ingress_network_filter <conf_server_common_ingress_network_filter>`
* :ref:`tcp_copy_buffer_size <conf_server_common_tcp_copy_buffer_size>`
* :ref:`tcp_copy_yield_size <conf_server_common_tcp_copy_yield_size>`
* :ref:`tcp_misc_opts <conf_server_common_tcp_misc_opts>`
* :ref:`tls_ticketer <conf_server_common_tls_ticketer>`
* :ref:`task_idle_check_duration <conf_server_common_task_idle_check_duration>`
* :ref:`task_idle_max_count <conf_server_common_task_idle_max_count>`
* :ref:`extra_metrics_tags <conf_server_common_extra_metrics_tags>`

listen
------

**optional**, **type**: :ref:`tcp listen <conf_value_tcp_listen>`

Set the listen config for this server.

The instance count setting will be ignored if *listen_in_worker* is correctly enabled.

**default**: not set

client_hello_recv_timeout
-------------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the timeout value for the wait of initial client hello data.

**default**: 10s

spawn_task_unconstrained
------------------------

**optional**, **type**: bool

Set if we should spawn tasks in tokio unconstrained way.

**default**: false

virtual_hosts
-------------

**required**, **type**: :ref:`host matched object <conf_value_host_matched_object>` <:ref:`host <configuration_server_rustls_proxy_host>`>

Set the list of hosts we should handle based on host match rules.

If not set, all requests will be handled.

Example:

.. code-block:: yaml

  hosts:
    name: bench
    exact_match: bench.example.net
    cert_pairs:
      certificate: bench.example.net-ec256.crt
      private_key: bench.example.net-ec256.key
    backends:
      - http

**default**: not set

.. _configuration_server_rustls_proxy_host:

Host
^^^^

This set the config for a OpenSSl virtual host.

name
""""

**required**, **type**: :ref:`metrics name <conf_value_metrics_name>`

Set the name of this virtual host.

**default**: not set

cert_pairs
""""""""""

**optional**, **type**: :ref:`tls cert pair <conf_value_tls_cert_pair>` or seq

Set certificate and private key pairs for this TLS server.

If not set, TLS protocol will be disabled.

**default**: not set

enable_client_auth
""""""""""""""""""

**optional**, **type**: bool

Set if you want to enable client auth.

**default**: disabled

no_session_ticket
"""""""""""""""""

**optional**, **type**: bool

Set if we should disable TLS session ticket (stateless session resumption by Session Ticket).

**default**: false

.. versionadded:: 0.3.3

no_session_cache
""""""""""""""""

**optional**, **type**: bool

Set if we should disable TLS session cache (stateful session resumption by Session ID).

**default**: false

.. versionadded:: 0.3.3

ca_certificate
""""""""""""""

**optional**, **type**: :ref:`tls certificates <conf_value_tls_certificates>`

A list of certificates for client auth. If not set, the system default ca certificates will be used.

**default**: not set

accept_timeout
""""""""""""""

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the timeout value for the accept of the full TLS handshake.

**default**: 10s

request_rate_limit
""""""""""""""""""

**optional**, **type**: :ref:`rate limit quota <conf_value_rate_limit_quota>`

Set rate limit on request.

**default**: no limit

request_max_alive
"""""""""""""""""

**optional**, **type**: usize, **alias**: request_alive_max

Set max alive requests at virtual host level.

Even if not set, the max alive requests should not be more than usize::MAX.

**default**: no limit

tcp_sock_speed_limit
""""""""""""""""""""

**optional**, **type**: :ref:`tcp socket speed limit <conf_value_tcp_sock_speed_limit>`

Set speed limit for each tcp socket.

This will overwrite the server level :ref:`tcp_sock_speed_limit <conf_server_common_tcp_sock_speed_limit>`.

**default**: no set

task_idle_max_count
"""""""""""""""""""

**optional**, **type**: i32

The task will be closed if the idle check return IDLE the times as this value.

This will overwrite the server level :ref:`task_idle_max_count <conf_server_common_task_idle_max_count>`.

**default**: not set

backends
""""""""
