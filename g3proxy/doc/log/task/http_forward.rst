.. _log_task_http_forward:

************
Http Forward
************

All config keys for TcpConnect task log also applies to HttpForward task log,
see :ref:`TcpConnect <log_task_tcp_connect>` for details.

The following keys are available only for HttpForward task log:

pipeline_wait
-------------

**required**, **type**: time duration string

Show the time spent from the receive of the http request header to the creation of the task.

reuse_connection
----------------

**required**, **type**: bool

Show if this task reuse old remote connection.

method
------

**required**, **type**: http method string

Show the http method string of the client request.

uri
---

**required**, **type**: http uri string

Show the uri of the client request. All non-printable characters will be escaped.

The max allowed number of characters of the uri is configurable at
:ref:`server <config_server_http_proxy_log_uri_max_chars>` or :ref:`user <config_user_log_uri_max_chars>` level.

user_agent
----------

**optional**, **type**: string

Show the first User-Agent header value in the client request.

rsp_status
----------

**optional**, **type**: int

Show the status code in the response that we send to the client.

origin_status
-------------

**optional**, **type**: int

Show the status code in the response we receive from the remote peer.

dur_req_send_hdr
----------------

**optional**, **type**: time duration string

Show the time spent from the creation of the task to when we sent out the request header to the remote peer.

dur_req_send_all
----------------

**optional**, **type**: time duration string

Show the time spent from the creation of the task to when we sent out the total request to the remote peer.

dur_rsp_recv_hdr
----------------

**optional**, **type**: time duration string

Show the time spent from the creation of the task to when we received the response header from the remote peer.

dur_rsp_recv_all
----------------

**optional**, **type**: time duration string

Show the time spent from the creation of the task to when we received the total response from the remote peer.
