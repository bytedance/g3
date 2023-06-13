.. _configuration_escaper:

*******
Escaper
*******

The type for each escaper config is *map*, with two always required keys:

* *name*, which specify the name of the escaper.
* *type*, which specify the real type of the escaper, decides how to parse other keys.

There are many types of escaper, each with a section below.

Escapers
========

.. toctree::
   :maxdepth: 2

   dummy_deny
   direct_fixed
   direct_float
   proxy_float
   proxy_http
   proxy_https
   proxy_socks5
   route_mapping
   route_query
   route_resolved
   route_select
   route_upstream
   route_client
   route_failover
   trick_float

Common Keys
===========

This section describes the common keys, they may be used by many escapers.

.. _conf_escaper_common_shared_logger:

shared_logger
-------------

**optional**, **type**: ascii

Set the escaper to use a logger running on a shared thread.

**default**: not set

.. _conf_escaper_common_resolver:

resolver
--------

**type**: str

Set the resolver to use for this escaper.

If the specified resolver doesn't exist in configure, a default DenyAll resolver will be used.

.. _conf_escaper_common_resolve_strategy:

resolve_strategy
-----------------

**optional**, **type**: :ref:`resolve strategy <conf_value_resolve_strategy>`

Set the resolve strategy.

.. _conf_escaper_common_tcp_sock_speed_limit:

tcp_sock_speed_limit
--------------------

**optional**, **type**: :ref:`tcp socket speed limit <conf_value_tcp_sock_speed_limit>`

Set speed limit for each tcp socket.

**default**: no limit, **alias**: tcp_conn_speed_limit | tcp_conn_limit

.. versionchanged:: 1.4.0 changed name to tcp_sock_speed_limit

.. _conf_escaper_common_udp_sock_speed_limit:

udp_sock_speed_limit
--------------------

**optional**, **type**: :ref:`udp socket speed limit <conf_value_udp_sock_speed_limit>`

Set speed limit for each udp socket.

**default**: no limit, **alias**: udp_relay_speed_limit | udp_relay_limit

.. versionchanged:: 1.4.0 changed name to udp_sock_speed_limit

.. _conf_escaper_common_no_ipv4:

no_ipv4
-------

**optional**, **type**: bool

Disable IPv4. This setting should be compatible with :ref:`resolve_strategy <conf_escaper_common_resolve_strategy>`.

**default**: false

.. _conf_escaper_common_no_ipv6:

no_ipv6
-------

**optional**, **type**: bool

Disable IPv6. This setting should be compatible with :ref:`resolve_strategy <conf_escaper_common_resolve_strategy>`.

**default**: false

.. _conf_escaper_common_tcp_connect:

tcp_connect
-----------

**optional**, **type**: :ref:`tcp connect <conf_value_tcp_connect>`

Set tcp connect params.

.. note:: For *direct* type escapers, the user level tcp connect params will be taken to limit the final value.

.. _conf_escaper_common_tcp_misc_opts:

tcp_misc_opts
-------------

**optional**, **type**: :ref:`tcp misc sock opts <conf_value_tcp_misc_sock_opts>`

Set misc tcp socket options.

**default**: not set, nodelay is default enabled

.. _conf_escaper_common_udp_misc_opts:

udp_misc_opts
-------------

**optional**, **type**: :ref:`udp misc sock opts <conf_value_udp_misc_sock_opts>`

Set misc udp socket options.

**default**: not set

.. _conf_escaper_common_default_next:

default_next
------------

**required**, **type**: str

Set the default next escaper for *route* type escapers.

.. _conf_escaper_common_pass_proxy_userid:

pass_proxy_userid
-----------------

**optional**, **type**: bool

Set if we should pass userid (username) to next proxy.

If set, the native basic auth method will be used when negotiation with next proxy, and the username field will be set
to the real username, the password field set to our package name (g3proxy if not forked).

**default**: false

.. note:: This will conflict with the real auth of next proxy.

.. _conf_escaper_common_use_proxy_protocol:

use_proxy_protocol
------------------

**optional**, **type**: :ref:`proxy protocol version <conf_value_proxy_protocol_version>`

Set the version of PROXY protocol we use for outgoing tcp connections.

**default**: not set, which means PROXY protocol won't be used

.. _conf_escaper_common_peer_negotiation_timeout:

peer_negotiation_timeout
------------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the negotiation timeout for next proxy peers.

**default**: 10s

.. _conf_escaper_common_extra_metrics_tags:

extra_metrics_tags
------------------

**optional**, **type**: :ref:`static metrics tags <conf_value_static_metrics_tags>`

Set extra metrics tags that should be added to escaper stats and user stats already with escaper tags added.

**default**: not set
