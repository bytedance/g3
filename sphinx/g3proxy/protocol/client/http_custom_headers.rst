.. _protocol_client_http_custom_headers:

###################
Http Custom Headers
###################

Chained Final Info Headers
==========================

We won't reset or append new values. So we can make sure the value is set by the proxy that is nearest to upstream.

X-BD-Upstream-Id
----------------

If this header is set, it means the response is from remote. The value for this value is *server_id*.

If we see this header in error response, we can know that the response is coming from the server after the one with the
same *server_id*, which may be upstream server, or another chained proxy server.

This header is controlled by http_proxy server option
:ref:`http_forward_mark_upstream <config_server_http_proxy_http_forward_mark_upstream>`.

X-BD-Upstream-Addr
------------------

If set, it will contains the remote address we are trying connect to from the far-most proxy server.

This header is controlled by http_proxy server option
:ref:`echo_chained_info <config_server_http_proxy_echo_chained_info>`.

X-BD-Outgoing-Ip
----------------

If set, it will contains the local bind ip address we are using to connect to remote from the far-most proxy server.

This header is controlled by http_proxy server option
:ref:`echo_chained_info <config_server_http_proxy_echo_chained_info>`.

Local Info Headers
==================

Every proxy configured will append new values. The value comes first if the proxy is nearer to upstream.

X-BD-Remote-Connection-Info
---------------------------

The value format:

::

    <server_id>[; bind=<bind_ip>][; remote=<remote_addr>][; local=<local_addr>][; expire=<expire_rfc3339>]

* bind_ip

  the ip address we decide to bind to before connection.

* remote_addr

  the socket address we decide to connect to before connection.

* local_addr

  the local socket address we bound to after the connection established.

* expire_rfc3339

  expire time of the remote peer. This field won't be set if the remote side is the target upstream.

This header is controlled by http_proxy server option :ref:`server_id <config_server_http_proxy_server_id>`.

X-BD-Dynamic-Egress-Info
------------------------

The value format:

::

    <server_id>[; isp=<isp>][; ip=<ip>][; area=<area>]

* isp

  ISP for the egress ip address.

* ip

  The egress ip address from external view.

* area

  Area of the egress ip address. The format is strings joined with '/', like 中国/山东/济南.
