.. _configuration_server:

******
Server
******

The type for each server config is *map*, with two always required keys:

* :ref:`name <conf_server_common_name>`, which specify the name of the server.
* :ref:`type <conf_server_common_type>`, which specify the real type of the server, decides how to parse other keys.

There are many types of server, each with a section below.

Servers
=======

.. toctree::
   :maxdepth: 1

   dummy_close
   tcp_stream
   tcp_tproxy
   tls_stream
   http_proxy
   socks_proxy
   http_rproxy
   sni_proxy
   plain_tcp_port
   plain_tls_port
   native_tls_port
   plain_quic_port
   intelli_proxy

Common Keys
===========

This section describes the common keys, they may be used by many servers.

.. _conf_server_common_name:

name
----

**required**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

Set the name of the server.

.. _conf_server_common_type:

type
----

**required**, **type**: str

Set the type of the server.

.. _conf_server_common_escaper:

escaper
-------

**required**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

Set the escaper to use with this server.

If the specified escaper doesn't exist in configure, a default DummyDeny escaper will be used.

.. _conf_server_common_auditor:

auditor
-------

**optional**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

Set the auditor to use with this server.

If the specified auditor doesn't exist in configure, a default auditor will be used.

.. _conf_server_common_user_group:

user_group
----------

**optional**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

Set the user group for auth.

If the specified user group doesn't exist in configure, a default user group with no users will be used.

**default**: no auth enabled

.. _conf_server_common_shared_logger:

shared_logger
-------------

**optional**, **type**: ascii

Set the server to use a logger running on a shared thread.

**default**: not set

.. _conf_server_common_listen_in_worker:

listen_in_worker
----------------

**optional**, **type**: bool

Set if we should listen in each worker runtime if you have worker enabled.

The listen instance count will be the same with the worker number count.

**default**: false

.. _conf_server_common_tls_server:

tls_server
----------

**optional**, **type**: :ref:`rustls server config <conf_value_rustls_server_config>`

Enable TLS on the listening socket and set TLS parameters.

**default**: disabled

.. _conf_server_common_tls_ticketer:

tls_ticketer
------------

**optional**, **type**: :ref:`tls ticketer <conf_value_tls_ticketer>`

Set a (remote) rolling TLS ticketer.

**default**: not set

.. versionadded:: 1.9.9

.. _conf_server_common_ingress_network_filter:

ingress_network_filter
----------------------

**optional**, **type**: :ref:`ingress network acl rule <conf_value_ingress_network_acl_rule>`

Set the network filter for clients.

The used client address will always be the interpreted client address, which means it will be the raw socket peer addr
for servers that listen directly, and it will be the address set in the PROXY Protocol message for serverw chained after
the server that support PROXY Protocol.

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

**default**: no limit

tcp_conn_speed_limit
--------------------

**deprecated**

.. versionchanged:: 1.11.8 deprecated, use tcp_sock_speed_limit instead

tcp_conn_limit
--------------

**deprecated**

.. versionchanged:: 1.11.8 deprecated, use tcp_sock_speed_limit instead

conn_limit
----------

**deprecated**

.. versionchanged:: 1.11.8 deprecated, use tcp_sock_speed_limit instead

.. _conf_server_common_udp_sock_speed_limit:

udp_sock_speed_limit
--------------------

**optional**, **type**: :ref:`udp socket speed limit <conf_value_udp_sock_speed_limit>`

Set speed limit for each udp socket.

**default**: no limit

udp_relay_speed_limit
---------------------

**deprecated**

.. versionchanged:: 1.11.8 deprecated, use udp_sock_speed_limit instead

udp_relay_limit
---------------------

**deprecated**

.. versionchanged:: 1.11.8 deprecated, use udp_sock_speed_limit instead

relay_limit
-----------

**deprecated**

.. versionchanged:: 1.11.8 deprecated, use udp_sock_speed_limit instead

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

.. _conf_server_common_udp_relay_batch_size:

udp_relay_batch_size
--------------------

**optional**, **type**: usize

Set the batch recvmsg / sendmsg size.

**default**: 8

.. versionadded:: 1.7.29

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

task_idle_check_duration
------------------------

**deprecated**

.. versionchanged:: 1.11.3 change default value from 5min to 60s
.. versionchanged:: 1.13.0 deprecated, use `task_idle_check_interval` instead

.. _conf_server_common_task_idle_check_interval:

task_idle_check_interval
------------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the idle check duration for task. The value will be up bound to seconds.

**default**: 60s, **max**: 30min, **min**: 2s

.. versionadded:: 1.13.0

.. _conf_server_common_task_idle_max_count:

task_idle_max_count
-------------------

**optional**, **type**: usize

The task will be closed if the idle check return IDLE the times as this value.

.. note:: The value set at user side will overwrite this.

**default**: 5

.. versionchanged:: 1.11.3 change default value from 1 to 5

.. _conf_server_common_flush_task_log_on_created:

flush_task_log_on_created
-------------------------

**optional**, **type**: bool

Log when task get created.

**default**: false

.. versionadded:: 1.11.0

.. _conf_server_common_flush_task_log_on_connected:

flush_task_log_on_connected
---------------------------

**optional**, **type**: bool

Log when upstream connected.

**default**: false

.. versionadded:: 1.11.0

.. _conf_server_common_task_log_flush_interval:

task_log_flush_interval
-----------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Enable periodic task log and set the flush interval.

.. note::

  There will be no periodic task log if protocol inspection is enabled, as intercept and inspect logs will be available
  in this case.

**default**: not set

.. versionadded:: 1.11.0

.. _conf_server_common_extra_metrics_tags:

extra_metrics_tags
------------------

**optional**, **type**: :ref:`static metrics tags <conf_value_static_metrics_tags>`

Set extra metrics tags that should be added to server stats and user stats already with server tags added.

**default**: not set

.. _config_server_username_params:

Username Params
===============

The username params can be used to extract egress path selection upstream addr, see :ref:`upstream addr <proto_egress_path_selection_upstream_addr>`.

The config value should be a map, the keys are:

keys_for_host
-------------

**optional**, **type**: list of string

Ordered keys that will be used to form the host label

require_hierarchy
-----------------

**optional**, **type**: bool

Require that if a later key appears, all its ancestors (earlier keys) must also appear

**default**: true

floating_keys
-------------

**optional**, **type**: list of string

Keys that can appear independently without requiring earlier keys (e.g., a generic optional key)

reject_unknown_keys
-------------------

**optional**, **type**: bool

Reject unknown keys not present in `keys_for_host`

**default**: true

reject_duplicate_keys
---------------------

**optional**, **type**: bool

Reject duplicate keys

**default**: true

separator
---------

**optional**, **type**: string

Separator used between labels

**default**: "-"

domain_suffix
-------------

**optional**, **type**: string

Optional domain suffix appended to computed host (e.g., ".svc.local")

**default**: not set

http_port
---------

**optional**: **type**: u16

Default port for HTTP proxy upstream selection

**default**: 10000

socks5_port
-----------

**optional**: **type**: u16

Default port for Socks5 proxy upstream selection

**default**: 10000

strip_suffix_for_auth
---------------------

**optional**, **type**: bool

If true, only the base part before '+' is used for auth username

**default**: true

.. versionadded:: 1.13.0
