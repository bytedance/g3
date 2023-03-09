.. _configuration_server_http_rproxy:

http_rproxy
===========

This server provides http reverse proxy.

The following common keys are supported:

* :ref:`escaper <conf_server_common_escaper>`
* :ref:`auditor <conf_server_common_auditor>`
* :ref:`user_group <conf_server_common_user_group>`
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

The auth scheme supported by the server is determined by the type of the specified user group.

+-------------+---------------------------+-------------------+
|auth scheme  |user group type            |is supported       |
+=============+===========================+===================+
|Basic        |hashed_user                |yes                |
+-------------+---------------------------+-------------------+
|Negotiate    |gss_api                    |not yet            |
+-------------+---------------------------+-------------------+

.. _config_server_http_rproxy_server_id:

server_id
---------

**optional**, **type**: :ref:`http server id <conf_value_http_server_id>`

Set the server id. If set, the header *X-BD-Remote-Connection-Info* will be added to response.

**default**: not set

auth_realm
----------

**optional**, **type**: :ref:`ascii str <conf_value_ascii_str>`

Set the auth realm.

**default**: proxy

req_header_recv_timeout
-----------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the max time to wait a full request header after the client connection become readable.

**default**: 30s

rsp_header_recv_timeout
-----------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the max time duration after the full request sent and before receive of the whole response header.

**default**: 60s

req_header_max_size
-------------------

**optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

Set the max request header size.

**default**: 64KiB

rsp_header_max_size
-------------------

**optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

Set the max response header size.

**default**: 64KiB

.. _config_server_http_rproxy_log_uri_max_chars:

log_uri_max_chars
-----------------

**optional**, **type**: usize

Set the max number of characters of uri should be logged in logs.

The user level config value will take effect if set, see this :ref:`user config option <config_user_log_uri_max_chars>`.

**default**: 1024

pipeline_size
-------------

**optional**, **type**: int

Set the pipeline size for HTTP 1.0/1.1.

**default**: 10

.. note::

  We only pipeline requests with no body.

pipeline_read_idle_timeout
--------------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the idle timeout of the client side IDLE http connections.

**default**: 5min

no_early_error_reply
--------------------

**optional**, **type**: bool

Set to true if no error reply should be sent out before user auth succeeded, the connection will be just closed
in such case.

**default**: false

body_line_max_length
--------------------

**optional**, **type**: int

Set the max line length for lines (trailer and chunk size) in http body.

**default**: 8192

http_forward_upstream_keepalive
-------------------------------

**optional**, **type**: :ref:`http keepalive <conf_value_http_keepalive>`

Set http keepalive config at server level.

**default**: set with default value

untrusted_read_speed_limit
--------------------------

**optional**, **type**: :ref:`tcp socket speed limit <conf_value_tcp_sock_speed_limit>`

Enable untrusted read of the body of requests with no auth info, and set the read rate limit.

Set this if you need to be compatible with buggy java http clients which won't handle the 407 error response in time.

**default**: not set, which means untrusted read is disabled, **alias**: untrusted_read_limit

.. versionchanged:: 1.4.0 changed name to untrusted_read_speed_limit

append_forwarded_for
--------------------

**optional**, **type**: :ref:`http forwarded header type <conf_value_http_forwarded_header_type>`

Set if we should append a corresponding forwarded header to the request send out to the next proxy.

See :ref:`steal_forwarded_for <config_server_http_proxy_steal_forwarded_for>` config option in http_proxy for more info
if you want to delete existing forwarded headers.

See the doc of supported escapers for detailed protocol info.

**default**: classic, which means *X-Forwarded-\** headers will be appended

hosts
-----

**required**, **type**: :ref:`host matched object <conf_value_host_matched_object>` <:ref:`host <configuration_server_http_rproxy_host>`>

Set the hosts we should handle based on host match rules.

Example 1:

.. code-block:: yaml

  hosts:
    services:
      upstream: www.example.net

Example 2:

.. code-block:: yaml

  hosts:
    - exact_match:
        - www.example.net
        - example.net
      services:
        upstream: www.example.net
    - child_match: example.org
      set_default: true
      services:
        upstream: www.example.org

**default**: not set

.. _configuration_server_http_rproxy_host:

Host
^^^^

This is the config for each local host on this server.

services
""""""""

**required**, **type**: :ref:`uri path matched object <conf_value_uri_path_matched_object>` <:ref:`service <configuration_server_http_rproxy_service>`>

Set the sites we should handle based on url path match rules.

tls_server
""""""""""

**optional**, **type**: :ref:`rustls server config <conf_value_rustls_server_config>`

Set TLS server config for this local site.

If not set, the :ref:`global tls server <configuration_server_http_rproxy_global_tls_server>` config will be used.

**default**: not set

.. _configuration_server_http_rproxy_service:

Service
^^^^^^^

This set the config for a upstream http service.

upstream
""""""""

**required**, **type**: :ref:`upstream str <conf_value_upstream_str>`

Set the target upstream address. The default port is 80 which can be omitted.

tls_client
""""""""""

**optional**, **type**: :ref:`openssl tls client config <conf_value_openssl_tls_client_config>`

Set TLS parameters for this local TLS client if https is needed.
If set to empty map, a default config is used.

**default**: not set

tls_name
""""""""

**optional**, **type**: :ref:`tls name <conf_value_tls_name>`

Set the tls server name to verify tls certificate of the upstream site.

If not set, the host part of the upstream address will be used.

**default**: not set

enable_tls_server
-----------------

**optional**, **type**: bool

Set whether tls is enabled for all local sites.

Requests to local sites without valid tls server config will be dropped.

**default**: false

.. _configuration_server_http_rproxy_global_tls_server:

global_tls_server
-----------------

**optional**, **type**: :ref:`rustls server config <conf_value_rustls_server_config>`

Set global TLS server config on the server. This will be used if no tls server config set on the matched local site.

**default**: not set

client_hello_recv_timeout
-------------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the timeout value for the receive of the complete TLS ClientHello message.

**default**: 1s
