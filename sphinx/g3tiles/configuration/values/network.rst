
.. _configure_network_value_types:

*******
Network
*******

.. _conf_value_sockaddr_str:

sockaddr str
============

**yaml value**: str

The string should be in *<ip>[:<port>]* format, in which the port may be omitted if a default value is available.

.. _conf_value_static_sockaddr_str:

static sockaddr str
===================

**yaml value**: str

The string should be in *@<domain>:<port>* or *@<ip>:<port>* format.

It is different from :ref:`upstream str <conf_value_upstream_str>` as:

- It will be resolved when we load the config files
- The domain is only allowed to be resolved to just 1 IP address

.. _conf_value_env_sockaddr_str:

env sockaddr str
================

**yaml value**: :ref:`sockaddr str <conf_value_sockaddr_str>` or :ref:`static sockaddr str <conf_value_static_sockaddr_str>` or :ref:`env var <conf_value_env_var>`

The string should be in *<ip>[:<port>]* format, in which the port may be omitted if a default value is available.

.. _conf_value_ip_addr_str:

ip addr str
===========

**yaml value**: str

The string should be in *<ip>* format.

.. _conf_value_ipv4_addr_str:

ipv4 addr str
=============

**yaml value**: str

The string should be in *<ipv4 address>* format.

.. _conf_value_ipv6_addr_str:

ipv6 addr str
=============

**yaml value**: str

The string should be in *<ipv6 address>* format.

Ipv4 mapped address should not be set when this type is required.

.. _conf_value_ip_network_str:

ip network str
==============

**yaml value**: str

The string should be a network address in CIDR format, or just an ip address.

.. _conf_value_host:

host
====

**yaml value**: str

A host value. Which should be either a valid domain, or a valid IP address.

.. _conf_value_domain:

domain
======

**yaml value**: str

A domain value. The string value should be able to convert to a IDNA domain.

Leading '.' is not allowed.

.. _conf_value_weighted_sockaddr:

weighted sockaddr
=================

**yaml value**: map | string

A socket addr str with weight set, which make can be grouped into selective vector.

The map consists 2 fields:

* addr

  **required**, **type**: :ref:`sockaddr str <conf_value_sockaddr_str>`

  The real value.

* weight

  **optional**, **type**: f64

  The weight of the real value.
  It may be converted to the smallest u32 greater than or equal to the f64 value when used.

  **default**: 1.0

If the value type is string, then it's value will be the *addr* field, with *weight* set to default value.

.. _conf_value_socket_buffer_config:

socket buffer config
====================

**yaml value**: mix

It consists of 2 fields:

* recv

  **optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

  Set the recv buf size.

  **default**: not set

* send

  **optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

  Set the send buf size.

  **default**: not set

The yaml value for *socket buffer config* can be in the following formats:

* int | string

  The value will be set for both **recv** and **send** fields above.

* map

  The keys of this map are the fields as described above.

.. _conf_value_connection_pool_config:

connection pool config
======================

**type**: map

The keys are:

* check_interval

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the min idle check interval.
  New connections will be established if the idle connections are less than *min_idle_count*.

  **default**: 10s

* max_idle_count

  **optional*, **type**: usize

  Set the maximum idle connections count.

  **default**: 1024

* min_idle_count

  **optional**, **type**: usize

  Set the minimum idle connections count.

  **default**: 32

* idle_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the keep timeout for IDLE connection.

  **default**: 5m

  .. versionadded:: 0.3.7

.. versionadded:: 0.3.5

.. _conf_value_tcp_listen:

tcp listen
==========

**yaml value**: mix

It consists of the following fields:

* address

  **required**, **type**: :ref:`env sockaddr str <conf_value_env_sockaddr_str>`

  Set the listen socket address.

  **default**: [::]:0, which has empty port

* backlog

  **optional**, **type**: unsigned int

  Set the listen backlog number for tcp sockets. The default value will be used if the specified value is less than 8.

  **default**: 4096

  .. note::

    If the backlog argument is greater than the value in /proc/sys/net/core/somaxconn, then it is silently truncated
    to that value. Since Linux 5.4, the default in this file is 4096; in earlier kernels, the default value is 128.

* netfilter_mark

  **optional**, **type**: unsigned int

  Set the netfilter mark (SOL_SOCKET, SO_MARK) value for the listening socket. If this field not present,
  the mark value will not be touch. This value can be used for advanced routing policy or netfilter rules.

* ipv6_only

  **optional**, **type**: bool

  Listen only to ipv6 address only if address is set to [::].

  **default**: false

* instance

  **optional**, **type**: int

  Set how many listen instances. If *scale* is set, this will be the least value.

  **default**: 1

* scale

  **optional**, **type**: float | string

  Set the listen instance count scaled according to available parallelism.

  For string value, it could be in percentage (n%) or fractional (n/d) format.

  Example:

  .. code-block:: yaml

    scale: 1/2
    # or
    scale: 0.5
    # or
    scale: 50%

  **default**: 0

The yaml value for *listen* can be in the following formats:

* int

  Set the port only.

* :ref:`sockaddr str <conf_value_sockaddr_str>`

  Set ip and port. The port field is required.

* map

  The keys of this map are the fields as described above.

.. _conf_value_tcp_connect:

tcp connect
===========

**yaml value**: map

This set TCP connect params.

It consists of 2 fields:

* max_retry

  **optional**, **type**: int

  Set the max tcp connect retry for a single upstream connection of the same address family.
  The total tcp connect tries will be *1 + max_retry*.

  Each resolved IP addr will be tried at most once.

  **default**: 2, which means the total tries is 3

* each_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the max timeout for each connection to the resolved addr of the upstream.

  **default**: 30s

.. _conf_value_udp_listen:

udp listen
==========

**yaml value**: mix

It consists of the following fields:

* address

  **required**, **type**: :ref:`env sockaddr str <conf_value_env_sockaddr_str>`

  Set the listen socket address.

  **default**: [::]:0, which has empty port

* ipv6_only

  **optional**, **type**: bool

  Listen only to ipv6 address only if address is set to [::].

  **default**: false

* socket_buffer

  **optional**, **type**: :ref:`socket buffer config <conf_value_socket_buffer_config>`

  Set an explicit socket buffer config.

  **default**: not set

* socket_misc_opts

  **optional**, **type**: :ref:`udp misc sock opts <conf_value_udp_misc_sock_opts>`

  Set misc UDP socket options.

  **default**: not set

* instance

  **optional**, **type**: int

  Set how many listen instances. If *scale* is set, this will be the least value.

  **default**: 1

* scale

  **optional**, **type**: float | string

  Set the listen instance count scaled according to available parallelism.

  For string value, it could be in percentage (n%) or fractional (n/d) format.

  Example:

  .. code-block:: yaml

    scale: 1/2
    # or
    scale: 0.5
    # or
    scale: 50%

  **default**: 0

The yaml value for *listen* can be in the following formats:

* int

  Set the port only.

* :ref:`sockaddr str <conf_value_sockaddr_str>`

  Set ip and port. The port field is required.

* map

  The keys of this map are the fields as described above.

.. _conf_value_happy_eyeballs:

happy eyeballs
==============

**yaml value**: map

This set Happy Eyeballs params for multiple tcp connections.

It consists of the following fields:

* resolution_delay

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  The resolution delay time for the wait of the preferred address family after another one is returned.

  **default**: 50ms

* second_resolution_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  The timeout time for the wait of the second resolution after no running connection attempts.

  **default**: 2s

* first_address_family_count

  **optional**, **type**: usize

  The address to try before use the addresses from another address family.

  **default**: 1

* connection_attempt_delay

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  The delay time before start a new connection after the previous one.

  **default**: 250ms, **min**: 100ms, **max**: 2s

.. _conf_value_tcp_keepalive:

tcp keepalive
=============

**yaml value**: mix

This set TCP level keepalive settings.

It consists of 2 fields:

* enable

  **optional**, **type**: bool

  Set whether tcp keepalive should be enabled.

  **default**: false, which means you can set limit on other values in case keepalive is needed somewhere

* idle_time

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the keepalive idle time.

  **default**: 60s

* probe_interval

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the probe interval after idle.

  **default**: not set, which means the OS default value will be used

* probe_count

  **optional**, **type**: u32

  Set the probe count.

  **default**: not set, which means the OS default value will be used

If the root value type is bool, the value will be parsed the same as the *enable* key.

If the root value type is not map and not bool, the value will be parsed the same as the *idle_time* key, but with
*enable* set to true.

.. _conf_value_tcp_misc_sock_opts:

tcp misc sock opts
==================

**yaml value**: map

This set misc tcp socket options.

Keys:

* no_delay

  **optional**, **type**: bool

  Set value for tcp level socket option TCP_NODELAY. If set to true, disable the Nagle algorithm.

  **default**: the default value varies, check the doc of the outer option

* mss

  **optional**, **type**: u32, **alias**: max_segment_size

  Set value for tcp level socket option TCP_MAXSEG, the maximum segment size for outgoing TCP packets.

  **default**: not set

* ttl

  **optional**, **type**: u32, **alias**: time_to_live

  Set value for ip level socket option IP_TTL, the time-to-live field in each sent packet.

  **default**: not set

* tos

  **optional**, **type**: u8, **alias**: type_of_service

  Set value for ip level socket option IP_TOS, the type-of-service field in each sent packet.

  **default**: not set

* mark

  **optional**, **type**: u32, **alias**: netfilter_mark

  Set value for socket level socket option SO_MARK, the netfilter mark value for our tcp sockets.

  **default**: not set

.. _conf_value_udp_misc_sock_opts:

udp misc sock opts
==================

**yaml value**: map

This set misc udp socket options.

Keys:

* ttl

  **optional**, **type**: u32, **alias**: time_to_live

  Set value for ip level socket option IP_TTL, the time-to-live field in each sent packet.

  **default**: not set

* tos

  **optional**, **type**: u8, **alias**: type_of_service

  Set value for ip level socket option IP_TOS, the type-of-service field in each sent packet.

  **default**: not set

* mark

  **optional**, **type**: u32, **alias**: netfilter_mark

  Set value for socket level socket option SO_MARK, the netfilter mark value for our tcp sockets.

  **default**: not set

.. _conf_value_http_header_name:

http header name
================

**yaml value**: str

This string should be a valid HTTP header name.

.. _conf_value_http_keepalive:

http keepalive
==============

**yaml value**: mix

This set HTTP level keepalive settings.

It consists of 2 fields:

* enable

  **optional**, **type**: bool

  Set whether tcp keepalive should be enabled.

  **default**: true

* idle_expire

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the idle expire time for the saved connection.
  If the last active time for the connection has elapsed, the connection will be dropped.

  **default**: 60s

If the root value type is bool, the value will be parsed the same as the *enable* key.

If the root value type is not map and not bool, the value will be parsed the same as the *idle_expire* key, but with
*enable* set to true.

.. _conf_value_http_forwarded_header_type:

http forwarded header type
==========================

**yaml value**: str | bool

This set the header type we set in requests for identifying the originating IP address of a client connected to us.

The string values are:

* none

  Do not set any header.

* classic

  Use the de-facto standard header *X-Forwarded-For*, this is widely used.

* standard

  Use the standard header *Forwarded* defined in rfc7239. We set both the *for* and the *by* parameter in this case.

If the yaml value type is bool, *true* will be *classic*, and *false* will be none.

.. _conf_value_http_server_id:

http server id
==============

**yaml value**: str

Set http server id (server name) for http forwarding services.

All characters should be ASCII in range '0x20' - '0x7E', except for ';' and ','.

.. _conf_value_proxy_protocol_version:

proxy protocol version
======================

**yaml value**: u8

Set the PROXY protocol version.

We support version 1 and version 2 for outgoing tcp connections.
