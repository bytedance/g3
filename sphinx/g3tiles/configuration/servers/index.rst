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
   :maxdepth: 2

   dummy_close
   openssl_proxy
   rustls_proxy
   keyless_proxy
   plain_tcp_port
   plain_quic_port

Common Keys
===========

This section describes the common keys, they may be used by many servers.

.. _conf_server_common_name:

**required**, **type**: :ref:`metrics name <conf_value_metrics_name>`

Set the name of the server.

.. _conf_server_common_type:

**required**, **type**: str

Set the type of the server.

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

.. _conf_server_common_ingress_network_filter:

ingress_network_filter
----------------------

**optional**, **type**: :ref:`ingress network acl rule <conf_value_ingress_network_acl_rule>`

Set the network filter for clients.

The used client address will always be the interpreted client address, which means it will be the raw socket peer addr
for servers that listen directly, and it will be the address set in the PROXY Protocol message for serverw chained after
the server that support PROXY Protocol.

**default**: not set

.. _conf_server_common_tcp_sock_speed_limit:

tcp_sock_speed_limit
--------------------

**optional**, **type**: :ref:`tcp socket speed limit <conf_value_tcp_sock_speed_limit>`

Set speed limit for each tcp socket.

**default**: no limit

.. _conf_server_common_udp_sock_speed_limit:

udp_sock_speed_limit
--------------------

**optional**, **type**: :ref:`udp socket speed limit <conf_value_udp_sock_speed_limit>`

Set speed limit for each udp socket.

**default**: no limit

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

.. _conf_server_common_tls_ticketer:

tls_ticketer
------------

**optional**, **type**: :ref:`tls ticketer <conf_value_tls_ticketer>`

Set a (remote) rolling TLS ticketer.

**default**: not set

.. versionadded:: 0.3.6

.. _conf_server_common_task_idle_check_duration:

task_idle_check_duration
------------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the idle check duration for task. The value will be up bound to seconds.

**default**: 60s, **max**: 30min, **min**: 2s

.. versionchanged:: 0.3.8 change default value from 5min to 60s

.. _conf_server_common_task_idle_max_count:

task_idle_max_count
-------------------

**optional**, **type**: usize

The task will be closed if the idle check return IDLE the times as this value.

**default**: 5

.. versionchanged:: 0.3.8 change default value from 1 to 5

.. _conf_server_common_extra_metrics_tags:

extra_metrics_tags
------------------

**optional**, **type**: :ref:`static metrics tags <conf_value_static_metrics_tags>`

Set extra metrics tags that should be added to server stats and user stats already with server tags added.

**default**: not set
