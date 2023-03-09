.. _configuration_server_socks_proxy:

socks_proxy
===========

This server provides socks proxy, which support tcp connect and udp associate.

The following common keys are supported:

* :ref:`escaper <conf_server_common_escaper>`
* :ref:`auditor <conf_server_common_auditor>`
* :ref:`user_group <conf_server_common_user_group>`
* :ref:`shared_logger <conf_server_common_shared_logger>`
* :ref:`listen <conf_server_common_listen>`
* :ref:`listen_in_worker <conf_server_common_listen_in_worker>`
* :ref:`tcp_sock_speed_limit <conf_server_common_tcp_sock_speed_limit>`
* :ref:`udp_sock_speed_limit <conf_server_common_udp_sock_speed_limit>`
* :ref:`ingress_network_filter <conf_server_common_ingress_network_filter>`
* :ref:`dst_host_filter_set <conf_server_common_dst_host_filter_set>`
* :ref:`dst_port_filter <conf_server_common_dst_port_filter>`
* :ref:`tcp_copy_buffer_size <conf_server_common_tcp_copy_buffer_size>`
* :ref:`tcp_copy_yield_size <conf_server_common_tcp_copy_yield_size>`
* :ref:`udp_relay_packet_size <conf_server_common_udp_relay_packet_size>`
* :ref:`udp_relay_yield_size <conf_server_common_udp_relay_yield_size>`
* :ref:`tcp_misc_opts <conf_server_common_tcp_misc_opts>`
* :ref:`udp_misc_opts <conf_server_common_udp_misc_opts>`
* :ref:`task_idle_check_duration <conf_server_common_task_idle_check_duration>`
* :ref:`task_idle_max_count <conf_server_common_task_idle_max_count>`
* :ref:`extra_metrics_tags <conf_server_common_extra_metrics_tags>`

The auth type supported by the server is determined by the type of the specified user group.

+-------------+---------------------------+-------------------+
|auth scheme  |user group type            |is supported       |
+=============+===========================+===================+
|user         |hashed_user                |yes                |
+-------------+---------------------------+-------------------+
|gssapi       |gss_api                    |not yet            |
+-------------+---------------------------+-------------------+

use_udp_associate
-----------------

**optional**, **type**: bool, **alias**: enable_udp_associate

Set whether we should use udp associate instead of udp connect.

**default**: false

negotiation_timeout
-------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the max time duration for negotiation, before we start to handle the real socks commands.

**default**: 4s

udp_client_initial_timeout
--------------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the max time duration to wait before the first udp packet after we send back the udp port info.

**default**: 30s

udp_bind_ipv4
-------------

**optional**, **type**: :ref:`list <conf_value_list>` of :ref:`ipv4 addr str <conf_value_ipv4_addr_str>`

Set the ipv4 addresses for udp associate local binding to socks client.
If not set, the server ip for the tcp connection will be used when setup the udp listen socket.

If set, the tcp connect can be in ipv6 address family.

**default**: not set

udp_bind_ipv6
-------------

**optional**, **type**: :ref:`list <conf_value_list>` of :ref:`ipv6 addr str <conf_value_ipv6_addr_str>`

Set the ipv6 addresses for udp associate local binding to socks client.
If not set, the server ip for the tcp connection will be used when setup the udp listen socket.

If set, the tcp connect can be in ipv4 address family.

**default**: not set

udp_bind_port_range
-------------------

**optional**, **type**: :ref:`port range <conf_value_port_range>`

Set the UDP port-range for udp associate local binding to socks client.
If not set, the port will be selected by the OS.

udp_socket_buffer
-----------------

**optional**, **type**: :ref:`socket buffer config <conf_value_socket_buffer_config>`

Set the buffer config for the udp socket.

.. note:: The buffer size of the socket at escaper side will also be set.

**default**: not set

auto_reply_local_ip_map
-----------------------

**optional**, **type**: map

Set this if you want to reply another ip other then the real bind ip for the udp listen socket to the client.

The key of the map should be the local ip, and the value should be the ip you want the client to use.

**default**: not set
