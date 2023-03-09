.. _configuration_server:

******
Server
******

The type for each server config is *map*, with two always required keys:

* *name*, which specify the name of the escaper.
* *type*, which specify the real type of the escaper, decides how to parse other keys.

There are many types of server, each with a section below.

Servers
=======

.. toctree::
   :maxdepth: 2

   dummy_close
   tcp_stream
   tls_stream
   http_proxy
   socks_proxy
   http_rproxy
   sni_proxy
   plain_tcp_port
   plain_tls_port
   intelli_proxy

Common Keys
===========

This section describes the common keys, they may be used by many escapers.

.. _conf_server_common_escaper:

escaper
-------

**required**, **type**: str

Set the escaper to use with this server.

If the specified escaper doesn't exist in configure, a default DummyDeny escaper will be used.

.. _conf_server_common_auditor:

auditor
-------

**optional**, **type**: str

Set the auditor to use with this server.

If the specified auditor doesn't exist in configure, a default auditor will be used.

.. versionadded:: 1.7.0

.. _conf_server_common_user_group:

user_group
----------

**optional**, **type**: str

Set the user group for auth.

If the specified user group doesn't exist in configure, a default user group with no users will be used.

**default**: no auth enabled

.. _conf_server_common_shared_logger:

shared_logger
-------------

**optional**, **type**: ascii

Set the server to use a logger running on a shared thread.

**default**: not set

.. _conf_server_common_listen:

listen
------

**required**, **type**: :ref:`tcp listen <conf_value_tcp_listen>`

Set the listen config for this server.

The instance count setting will be ignored if *listen_in_worker* is correctly enabled.

.. _conf_server_common_listen_in_worker:

listen_in_worker
----------------

**optional**, **type**: bool

Set if we should listen in each worker runtime if you have worker enabled.

The listen instance count will be the same with the worker number count.

**default**: false

.. versionadded:: 1.7.8

.. _conf_server_common_tls_server:

tls_server
----------

**optional**, **type**: :ref:`rustls server config <conf_value_rustls_server_config>`

Enable TLS on the listening socket and set TLS parameters.

**default**: disabled

.. _conf_server_common_ingress_network_filter:

ingress_network_filter
----------------------

**optional**, **type**: :ref:`ingress network acl rule <conf_value_ingress_network_acl_rule>`

Set the network filter for clients.

**default**: not set

.. _conf_server_common_dst_host_filter_set:

dst_host_filter_set
-------------------

**optional**, **type**: :ref:`dst host acl rule set <conf_value_dst_host_acl_rule_set>`

Set the filter for dst host of each request.

.. note:: This won't limit the Host header in http protocol.

**default**: not set

.. _conf_server_common_dst_port_filter:

dst_port_filter
---------------

**optional**, **type**: :ref:`exact port acl rule <conf_value_exact_port_acl_rule>`

Set the filter for dst port of each request.

**default**: not set

.. _conf_server_common_tcp_sock_speed_limit:

tcp_sock_speed_limit
--------------------

**optional**, **type**: :ref:`tcp socket speed limit <conf_value_tcp_sock_speed_limit>`

Set speed limit for each tcp socket.

**default**: no limit, **alias**: tcp_conn_speed_limit | tcp_conn_limit

.. versionchanged:: 1.4.0 changed name to tcp_sock_speed_limit

.. _conf_server_common_udp_sock_speed_limit:

udp_sock_speed_limit
--------------------

**optional**, **type**: :ref:`udp socket speed limit <conf_value_udp_sock_speed_limit>`

Set speed limit for each udp socket.

**default**: no limit, **alias**: udp_relay_speed_limit | udp_relay_limit

.. versionchanged:: 1.4.0 changed name to udp_sock_speed_limit

.. _conf_server_common_tcp_copy_buffer_size:

tcp_copy_buffer_size
--------------------

**optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

Set the buffer size for internal tcp copy.

**default**: 16K, **minimal**: 4K

.. _conf_server_common_tcp_copy_yield_size:

tcp_copy_yield_size
-------------------

**optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

Set the yield out size for the internal copy task.

**default**: 1M, **minimal**: 256K

.. _conf_server_common_udp_relay_packet_size:

udp_relay_packet_size
---------------------

**optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

Set the udp packet size for udp relay.

**default**: 4K, **maximum**: 16K

.. _conf_server_common_udp_relay_yield_size:

udp_relay_yield_size
--------------------

**optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

Set the yield out size for the internal relay task.

**default**: 1M, **maximum**: 256K

.. _conf_server_common_tcp_misc_opts:

tcp_misc_opts
-------------

**optional**, **type**: :ref:`tcp misc sock opts <conf_value_tcp_misc_sock_opts>`

Set misc tcp socket options on accepted tcp sockets.

**default**: not set, nodelay is default enabled

.. _conf_server_common_udp_misc_opts:

udp_misc_opts
-------------

**optional**, **type**: :ref:`udp misc sock opts <conf_value_udp_misc_sock_opts>`

Set misc udp socket options on created udp sockets.

**default**: not set

.. _conf_server_common_task_idle_check_duration:

task_idle_check_duration
------------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the idle check duration for task.

**default**: 5min, **max**: 30min

.. _conf_server_common_task_idle_max_count:

task_idle_max_count
-------------------

**optional**, **type**: i32

The task will be closed if the idle check return IDLE the times as this value.

.. note:: The value set at user side will overwrite this.

**default**: 1

.. _conf_server_common_extra_metrics_tags:

extra_metrics_tags
------------------

**optional**, **type**: :ref:`static metrics tags <conf_value_static_metrics_tags>`

Set extra metrics tags that should be added to server stats and user stats already with server tags added.

**default**: not set
