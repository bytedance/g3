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

.. _conf_value_ip_network_str:

ip network str
==============

**yaml value**: str

The string should be a network address in CIDR format, or just an ip address.

.. _conf_value_interface_name:

interface name
==============

**yaml value**: str | u32

The string should be a network interface name or index.

.. _conf_value_host:

host
====

**yaml value**: str

A host value. Which should be either a valid domain, or a valid IP address.

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

.. _conf_value_udp_listen:

udp listen
==========

**yaml value**: mix

It consists of the following fields:

* address

  **required**, **type**: :ref:`env sockaddr str <conf_value_env_sockaddr_str>`

  Set the listen socket address.

  **default**: [::]:0, which has empty port

* interface

  **optional**: **type**: :ref:`interface name <conf_value_interface_name>`

  Bind the outgoing socket to a particular device like “eth0”.

  **default**: not set

* socket_buffer

  **optional**, **type**: :ref:`socket buffer config <conf_value_socket_buffer_config>`

  Set an explicit socket buffer config.

  **default**: not set

* socket_misc_opts

  **optional**, **type**: :ref:`udp misc sock opts <conf_value_udp_misc_sock_opts>`

  Set misc UDP socket options.

  **default**: not set

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

.. _conf_value_udp_misc_sock_opts:

udp misc sock opts
==================

**yaml value**: map

This set misc udp socket options.

Keys:

* time_to_live

  **optional**, **type**: u8, **alias**: ttl

  Set value for ip level socket option IP_TTL, the time-to-live field in each sent packet.

  **default**: not set

* hop_limit

  **optional**, **type**: u8

  Set value for ipv6 level socket option IPV6_UNICAST_HOPS, the hop limit field in each sent packet.

  **default**: not set

  .. versionadded:: 0.1.1

* type_of_service

  **optional**, **type**: u8, **alias**: tos

  Set value for ip level socket option IP_TOS, the type-of-service field in each sent packet.

  **default**: not set

* traffic_class

  **optional**, **type**: u8

  Set value for ipv6 level socket option IPV6_TCLASS, the traffic class field in each sent packet.

  **default**: not set

  .. versionadded:: 0.1.1

* netfilter_mark

  **optional**, **type**: u32, **alias**: mark

  Set value for socket level socket option SO_MARK, the netfilter mark value for our tcp sockets.

  **default**: not set

.. _conf_value_http_header_value:

http header value
=================

**yaml value**: str

This string should be a valid HTTP header value.
