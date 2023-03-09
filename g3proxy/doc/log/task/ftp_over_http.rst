.. _log_task_ftp_over_http:

*************
FTP Over HTTP
*************

The following keys are available for TcpConnect task log:

server_addr
-----------

**required**, **type**: socket address string

The listening address of the server.

client_addr
-----------

**required**, **type**: socket address string

The client address.

upstream
--------

**required**, **type**: domain:port | socket address string

The target upstream that the client want to access.

next_bind_ip
------------

**optional**, **type**: ip address string

The selected bind IP before we really connect to the remote peer.

Present only if bind ip config is enabled on the corresponding escaper.

next_expire
-----------

**optional**, **type**: rfc3339 timestamp string with microseconds

The expected expire time of the next peer.

Present only if the next escaper is dynamic and we have selected the remote peer.

ftp_c_bound_addr
----------------

**optional**, **type**: socket address string

The local address for the remote ftp control connection.

Present only if we have connected to the remote peer.

ftp_c_peer_addr
---------------

**optional**, **type**: socket address string

The peer address for the remote ftp control connection.

The peer may be the upstream, or will be a next proxy address, which depends on the type of escaper.

Present only if we have selected the ip address of the next peer.

ftp_c_connect_tries
-------------------

**optional**, **type**: int

How many times we have tried to connect to the remote peer to establish the ftp control connection.

ftp_c_connect_spend
-------------------

**optional**, **type**: time duration string

How many time we have spent during the ftp control connection of the remote peer (all tries count in).

ftp_d_bound_addr
----------------

**optional**, **type**: socket address string

The local address for the remote ftp data connection.

Present only if we have connected to the remote peer.

ftp_d_peer_addr
---------------

**optional**, **type**: socket address string

The peer address for the remote ftp data connection.

The peer may be the upstream, or will be a next proxy address, which depends on the type of escaper.

Present only if we have selected the ip address of the next peer.

ftp_d_connect_tries
-------------------

**optional**, **type**: int

How many times we have tried to connect to the remote peer to establish the ftp data connection.

ftp_d_connect_spend
-------------------

**optional**, **type**: time duration string

How many time we have spent during the ftp data connection of the remote peer (all tries count in).

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

c_rd_bytes
----------

**optional**, **type**: int

How many bytes we have received from client.

c_wr_bytes
----------

**optional**, **type**: int

How many bytes we have sent to client.

ftp_c_rd_bytes
--------------

**optional**, **type**: int

How many bytes we have received from the remote peer through the ftp control connection.

ftp_c_wr_bytes
--------------

**optional**, **type**: int

How many bytes we have sent to the remote peer through the ftp control connection.

ftp_d_rd_bytes
--------------

**optional**, **type**: int

How many bytes we have received from the remote peer through the ftp data connection.

ftp_d_wr_bytes
--------------

**optional**, **type**: int

How many bytes we have sent to the remote peer through the ftp data connection.
