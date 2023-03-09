.. _configuration_server_http_proxy:

http_proxy
==========

This server provides http proxy, including http forward and http connect.

The following common keys are supported:

* :ref:`escaper <conf_server_common_escaper>`
* :ref:`auditor <conf_server_common_auditor>`
* :ref:`user_group <conf_server_common_user_group>`
* :ref:`shared_logger <conf_server_common_shared_logger>`
* :ref:`listen <conf_server_common_listen>`
* :ref:`listen_in_worker <conf_server_common_listen_in_worker>`
* :ref:`tls_server <conf_server_common_tls_server>`
* :ref:`tcp_sock_speed_limit <conf_server_common_tcp_sock_speed_limit>`
* :ref:`ingress_network_filter <conf_server_common_ingress_network_filter>`
* :ref:`dst_host_filter_set <conf_server_common_dst_host_filter_set>`
* :ref:`dst_port_filter <conf_server_common_dst_port_filter>`
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

.. _config_server_http_proxy_server_id:

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

tls_client
----------

**optional**, **type**: :ref:`openssl tls client config <conf_value_openssl_tls_client_config>`

Set TLS client parameters for https forward requests.

**default**: set with default value

ftp_client
----------

**optional**, **type**: :ref:`ftp client config <conf_value_ftp_client_config>`

Set the ftp client config for FTP over Http requests.

**default**: set with default value

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

.. _config_server_http_proxy_log_uri_max_chars:

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

allow_custom_host
-----------------

**optional**, **type**: bool

Set if custom *Host* header is allowed. If set to false, the *Host* header in http headers should have the same domain
or ip address with the one in the request method line.

**default**: true

.. note:: we don't require the *Host* header to be present in http headers no matter what have been set for this

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

.. _config_server_http_proxy_http_forward_mark_upstream:

http_forward_mark_upstream
--------------------------

**optional**, **type**: bool

If set, the header *X-BD-Upstream-Id* header will be added to the response from upstream, with the value to be
:ref:`server_id <config_server_http_proxy_server_id>`.
Local generated response will not contains this header.

**default**: false

.. _config_server_http_proxy_echo_chained_info:

echo_chained_info
-----------------

**optional**, **type**: bool

Set whether to add custom header in response that provides chained information
about the direct connection to upstream.

The custom headers are:

- X-BD-Upstream-Addr
- X-BD-Outgoing-IP

**default**: false

untrusted_read_speed_limit
--------------------------

**optional**, **type**: :ref:`tcp socket speed limit <conf_value_tcp_sock_speed_limit>`

Enable untrusted read of the body of requests with no auth info, and set the read rate limit.

Set this if you need to be compatible with buggy java http clients which won't handle the 407 error response in time.

**default**: not set, which means untrusted read is disabled, **alias**: untrusted_read_limit

.. versionchanged:: 1.4.0 changed name to untrusted_read_speed_limit

.. _config_server_http_proxy_egress_path_selection_header:

egress_path_selection_header
----------------------------

**optional**, **type**: str, **alias**: path_selection_header

Set the http custom header name to be used for path selection.

**default**: not set

.. _config_server_http_proxy_steal_forwarded_for:

steal_forwarded_for
-------------------

**optional**, **type**: bool

Set if we should delete the *Forwarded* and *X-Forwarded-For* headers from the client's request.

**default**: false
