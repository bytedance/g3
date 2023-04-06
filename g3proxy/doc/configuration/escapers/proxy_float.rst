.. _configuration_escaper_proxy_float:

***********
proxy_float
***********

This escaper provide the capability to access the target upstream through dynamic remote proxies.

The following interfaces are supported:

* tcp connect
* http(s) forward

The following remote proxy protocols are supported:

* Http Proxy
* Socks5 Proxy

The Cap'n Proto RPC publish command is supported on this escaper, the published data should be an array of
or just one :ref:`peer <config_escaper_dynamic_peer>`.

There is no path selection support for this escaper.

Config Keys
===========

The following common keys are supported:

* :ref:`shared_logger <conf_escaper_common_shared_logger>`
* :ref:`tcp_sock_speed_limit <conf_escaper_common_tcp_sock_speed_limit>`
* :ref:`tcp_misc_opts <conf_escaper_common_tcp_misc_opts>`
* :ref:`peer negotiation timeout <conf_escaper_common_peer_negotiation_timeout>`
* :ref:`extra_metrics_tags <conf_escaper_common_extra_metrics_tags>`

source
------

**required**, **type**: :ref:`url str <conf_value_url_str>` | map | null

Set the fetch source for peers.

We support many type of sources. The type is detected by reading the *scheme* field of url,
or the *type* key of the map. See :ref:`sources <config_escaper_dynamic_source>` for all supported type of sources.

cache
-----

**recommend**, **type**: :ref:`file path <conf_value_file_path>`

Set the cache file.

It is recommended to set this as the fetch of peers at startup may be finished after the first batch of requests.

The file will be created if not existed.

**default**: not set

refresh_interval
----------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the refresh interval to update peers from the configured source.

**default**: 1s

bind_ipv4
---------

**optional**, **type**: :ref:`ipv4 addr str <conf_value_ipv4_addr_str>`

Set the bind ip address for inet sockets.

**default**: not set

bind_ipv6
---------

**optional**, **type**: :ref:`ipv6 addr str <conf_value_ipv6_addr_str>`

Set the bind ip address for inet6 sockets.

**default**: not set

tls_client
----------

**optional**, **type**: bool | :ref:`openssl tls client config <conf_value_openssl_tls_client_config>`

Enable https peer, and set TLS parameters for this local TLS client.
If set to true or empty map, a default config is used.

**default**: not set

tcp_connect_timeout
-------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the tcp connect application level timeout value.

**default**: 30s

tcp_keepalive
-------------

**optional**, **type**: :ref:`tcp keepalive <conf_value_tcp_keepalive>`

Set tcp keepalive.

The tcp keepalive set in user config won't be taken into account.

**default**: 60s

expire_guard_duration
---------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

If the peer has an expire value, we won't connect to it if we can reach the expire time after adding this value.

**default**: 5s

.. _config_escaper_dynamic_source:

Sources
=======

For *map* format, the **type** key should always be set.

passive
-------

Do not fetch peers. Only publish is needed.

The root value of source may be set to *null* to use passive source.

redis
-----

Fetch peers from a redis db.

The keys used in the *map* format are:

* addr

  **required**, **type**: :ref:`upstream str <conf_value_upstream_str>`

  Set the address of the redis instance. The default port is 6379 which can be omitted.

* db

  **optional**, **type**: int

  Set the database.

  **default**: 0

* username

  **optional**, **type**: str

  Set the username for redis 6 database if needed. It is required if connect to an ACL enabled redis 6 database.

  **default**: not set

* password

  **optional**, **type**: str

  Set the password.

  **default**: not set

* connect_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the connect timeout.

  **default**: 5s

* read_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout for redis read operation.

  **default**: 2s

* sets_key

  **required**, **type**: str

  Set the key for the sets that store the peers. Each string record in the set is a single peer.
  See :ref:`peers <config_escaper_dynamic_peer>` for its formats.

For *url* str values, the format is:

    redis://[username][:<password>@]<addr>/<db>?sets_key=<sets_key>

redis_cluster
-------------

Fetch peers from a redis cluster.

The value should be a *map*, with these keys:

* initial_nodes

  **required**, **type**: :ref:`upstream str <conf_value_upstream_str>`

  Set the address of the startup nodes.

* username

  **optional**, **type**: str

  Set the username.

  .. versionadded:: 1.7.0

* password

  **optional**, **type**: str

  Set the password.

  **default**: not set

* connect_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the connect timeout.

  **default**: 5s

  .. versionadded:: 1.7.12

* read_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout for redis read operation.

  **default**: 2s

* sets_key

  **required**, **type**: str

  Set the key for the sets that store the peers. Each string record in the set is a single peer.
  See :ref:`peers <config_escaper_dynamic_peer>` for its formats.

.. _config_escaper_dynamic_peer:

Peers
=====

We use json string to represent a peer, with a map type as root element.

Common keys
-----------

* type

  **required**, **type**: str

  It tells us the peer type.

* addr

  **required**, **type**: :ref:`sockaddr str <conf_value_sockaddr_str>`

  Set the socket address we can connect to the peer.
  No domain name is allowed here.

* isp

  **optional**, **type**: str

  ISP for the egress ip address.

* eip

  **optional**, **type**: :ref:`ip addr str <conf_value_ip_addr_str>`

  The egress ip address from external view.

* area

  **optional**, **type**: :ref:`egress area <conf_value_egress_area>`

  Area of the egress ip address.

* expire

  **optional**, **type**: :ref:`rfc3339 datetime str <conf_value_rfc3339_datetime_str>`

  Set the expire time for this peer.

* tcp_sock_speed_limit

  **optional**, **type**: :ref:`tcp socket speed limit <conf_value_tcp_sock_speed_limit>`

  Set the speed limit for each tcp connections to this peer.

  .. versionchanged:: 1.4.0 changed name to tcp_sock_speed_limit

The following types are supported:

http
----

* username

  **optional**, **type**: :ref:`username <conf_value_username>`

  Set the username for HTTP basic auth.

* password

  **optional**, **type**: :ref:`password <conf_value_password>`

  Set the password for HTTP basic auth.

* http_connect_rsp_header_max_size

  **optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

  Set the max header size for received CONNECT response.

  **default**: 4KiB

* extra_append_headers

  **optional**, **type**: map

  Set extra headers append to the requests sent to upstream.
  The key should be the header name, both the key and the value should be in ascii string type.

  .. note:: No duplication check is done here, use it with caution.


https
-----

* username

  **optional**, **type**: :ref:`username <conf_value_username>`

  Set the username for HTTP basic auth.

* password

  **optional**, **type**: :ref:`password <conf_value_password>`

  Set the password for HTTP basic auth.

* tls_name

  **optional**, **type**: :ref:`tls name <conf_value_tls_name>`

  Set the tls server name for server certificate verification.

  .. note:: IP address is not supported by now. So if not set, the connection will fail.

  **default**: not set

* http_connect_rsp_header_max_size

  **optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

  Set the max header size for received CONNECT response.

  **default**: 4KiB

* extra_append_headers

  **optional**, **type**: map

  Set extra headers append to the requests sent to upstream.
  The key should be the header name, both the key and the value should be in ascii string type.

  .. note:: No duplication check is done here, use it with caution.

socks5
------

* username

  **optional**, **type**: :ref:`username <conf_value_username>`

  Set the username for Socks5 User auth.

* password

  **optional**, **type**: :ref:`password <conf_value_password>`

  Set the password for Socks5 User auth.
