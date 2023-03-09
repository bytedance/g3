
.. _configure_network_value_types:

*******
Network
*******

.. _conf_value_sockaddr_str:

sockaddr str
============

**yaml value**: str

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

.. _conf_value_egress_area:

egress area
===========

**yaml value**: str

Area of the egress ip address. The format is strings joined with '/', like "中国/山东/济南".

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

.. _conf_value_ports:

ports
=====

**yaml value**: mix

A collection of ip ports. The base type may be:

* u16 port

  A single port.

* <start>-<end> port range

  A port range, which includes both *start* and *end*. *end* should be greater than *start*.

* comma separated discrete port(s)

  A list of port. Each could be a port or a range of ports.

The yaml value could be:

* int

  int base type.

* str

  str base types.

* array

  array of base types.

.. _conf_value_port_range:

port range
==========

**yaml value**: mix

A consequent range of ip ports. It consists of 2 fields:

* start

  **required**, **type**: u16, **inclusive**

  The start of the port range. Should be greater than zero.

* end

  **required**, **type**: u16, **inclusive**

  The end of the port range. Should be greater than *start*.

The yaml value for *port range* can be in the following formats:

* str

  In format <start>-<end>. Extra whitespaces is allowed.

* map

  The keys of this map are the fields as described above.

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

.. _conf_value_tcp_listen:

tcp listen
==========

**yaml value**: mix

It consists of 4 fields:

* address

  **required**, **type**: :ref:`sockaddr str <conf_value_sockaddr_str>`

  Set the listen socket address.

  **default**: [::]:0, which has empty port

* backlog

  **optional**, **type**: unsigned int

  Set the listen backlog number for tcp sockets. The default value will be used if the specified value is less than 8.

  **default**: 4096

  .. note::

    If the backlog argument is greater than the value in /proc/sys/net/core/somaxconn, then it is silently truncated
    to that value. Since Linux 5.4, the default in this file is 4096; in earlier kernels, the default value is 128.

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

  .. versionadded:: 1.7.8

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

.. versionadded:: 1.5.3

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

.. _conf_value_http_forward_capability:

http forward capability
=======================

**yaml value**: map

The following fields can be set:

* forward_https

  **optional**, **type**: bool

  Whether we should forward request of https url to next proxy.

  If not, we will do tls handshake with upstream locally.

  **default**: false

* forward_ftp

  **optional**, **type**: bool

  Whether we should forward all requests of ftp url to next proxy.

  If not, we will act as a ftp client.

  It can be overwritten by the specific forward_ftp_* options as described below for the corresponding http methods.

  **default**: false

* forward_ftp_get

  **optional**, **type**: bool

  Whether we should forward the GET request of ftp url to next proxy.

  If not, we will act as a ftp client.

  **default**: false

* forward_ftp_put

  **optional**, **type**: bool

  Whether we should forward the PUT request of ftp url to next proxy.

  If not, we will act as a ftp client.

  **default**: false

* forward_ftp_del

  **optional**, **type**: bool

  Whether we should forward the DELETE request of ftp url to next proxy.

  If not, we will act as a ftp client.

  **default**: false

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

.. _conf_value_ftp_control_config:

ftp control config
==================

**yaml value**: map

The following fields can be set:

* max_line_len

  **optional**, **type**: usize

  Set the max line length.

  **default**: 2048

* max_multi_lines

  **optional**, **type**: usize

  Set the max lines for multi-line reply.

  **default**: 128

* command_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the general command timeout value for commands with no explicit timeout config.

  **default**: 10s

.. _conf_value_ftp_transfer_config:

ftp transfer config
===================

**yaml value**: map

The following fields can be set:

* list_max_line_len

  **optional**, **type**: usize

  Set the max line length for list reply.

  **default**: 2048

* list_max_entries

  **optional**, **type**: usize

  Set the max lines will be handled in list reply.

  **default**: 1024

* list_all_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout value for listing.

  **default**: 120s, **max**: 300s

* end_wait_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout value when waiting for the end of the transfer action at both the control and the transfer channel.

  **default**: 10s

.. _conf_value_ftp_client_config:

ftp client config
=================

**yaml value**: map

The following fields can be set:

* control

  **optional**, **type**: :ref:`ftp control config <conf_value_ftp_control_config>`

  Set config for the ftp control channel.

* transfer

  **optional**, **type**: :ref:`ftp transfer config <conf_value_ftp_transfer_config>`

  Set config for the ftp transfer channels.

* connect_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the connection timeout for both control and transfer channels.

  **default**: 30s

* greeting_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout for waiting of the greeting message from the server.

  **default**: 10s

* always_try_epsv

  **optional**, **type**: bool

  Set if we should always try EPSV command even server doesn't set it in feature.

  **default**: true

.. _conf_value_dns_encryption_protocol:

dns encryption protocol
=======================

**yaml value**: enum

The followings values are supported:

* dns-over-tls | dot | tls

  If `dns over tls`_ should be used.

.. _dns over tls: https://datatracker.ietf.org/doc/html/rfc7858

* dns-over-https | doh | https

  If `dns over https`_ should be used.

.. _dns over https: https://datatracker.ietf.org/doc/html/rfc8484

.. _conf_value_dns_encryption_config:

dns encryption config
=====================

**yaml value**: map | str

The following fields can be set:

* tls_name

  **required**, **type**: :ref:`tls name <conf_value_tls_name>`

  Set the tls server name.

* protocol

  **optional**, **type**: :ref:`dns encryption protocol <conf_value_dns_encryption_protocol>`

  Set the encryption protocol.

  **default**: dns-over-tls

* tls_client

  **optional**, **type**: :ref:`rustls client config <conf_value_rustls_client_config>`

  Set the tls client config.

  .. note:: not all fields will be used, check the doc of each key has the value *dns encryption config*.

  **default**: not set

If in str format, the value will be treated as field *tls_name*.

.. versionadded:: 1.1.4

.. _conf_value_proxy_request_type:

proxy request type
==================

**yaml type**: enum string

The values are:

* HttpForward
* HttpsForward
* FtpOverHttp
* HttpConnect
* SocksTcpConnect
* SocksUdpAssociate
