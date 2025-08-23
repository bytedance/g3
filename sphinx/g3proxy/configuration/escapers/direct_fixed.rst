.. _configuration_escaper_direct_fixed:

direct_fixed
============

This escaper will access the target upstream from local machine directly.

The following interfaces are supported:

* tcp connect
* udp relay
* udp connect
* http(s) forward
* ftp over http

The following egress path selection methods is supported:

* :ref:`by index <proto_egress_path_selection_by_index>`

  The index will be used as the index of the binding IP address

The following common keys are supported:

* :ref:`shared_logger <conf_escaper_common_shared_logger>`
* :ref:`resolver <conf_escaper_common_resolver>`, **required**
* :ref:`resolve_strategy <conf_escaper_common_resolve_strategy>`

  The user custom resolve strategy will be taken into account.

* :ref:`tcp_sock_speed_limit <conf_escaper_common_tcp_sock_speed_limit>`
* :ref:`udp_sock_speed_limit <conf_escaper_common_udp_sock_speed_limit>`
* :ref:`bind_interface <conf_escaper_common_bind_interface>`
* :ref:`no_ipv4 <conf_escaper_common_no_ipv4>`
* :ref:`no_ipv6 <conf_escaper_common_no_ipv6>`
* :ref:`tcp_connect <conf_escaper_common_tcp_connect>`

  The user tcp connect params will be taken into account.

* :ref:`happy eyeballs <conf_escaper_common_happy_eyeballs>`
* :ref:`tcp_misc_opts <conf_escaper_common_tcp_misc_opts>`
* :ref:`udp_misc_opts <conf_escaper_common_udp_misc_opts>`
* :ref:`extra_metrics_tags <conf_escaper_common_extra_metrics_tags>`

bind_ip
-------

**optional**, **type**: :ref:`ip addr str <conf_value_ip_addr_str>` | seq

Set the bind ip address(es) for sockets.

For *seq* value, each of its element must be :ref:`ip addr str <conf_value_ip_addr_str>`.
Only random select is supported. Use *route* type escapers if is doesn't meet your needs.

**default**: not set

egress_network_filter
---------------------

**optional**, **type**: :ref:`egress network acl rule <conf_value_egress_network_acl_rule>`

Set the network filter for the (resolved) remote ip address.

**default**: all permitted except for loop-back and link-local addresses

tcp_keepalive
-------------

**optional**, **type**: :ref:`tcp keepalive <conf_value_tcp_keepalive>`

Set tcp keepalive.

The tcp keepalive set in user config will be taken into account.

**default**: no keepalive set

resolve_redirection
-------------------

**optional**, **type**: :ref:`resolve redirection <conf_value_resolve_redirection>`

Set the dns redirection rules at escaper level.

**default**: not set

enable_path_selection
---------------------

**optional**, **type**: bool

Weather we should enable path selection.

.. note:: Path selection on server side should be open, or this option will have no effects.

**default**: false

use_proxy_protocol
------------------

**optional**, **type**: :ref:`proxy protocol version <conf_value_proxy_protocol_version>`

Set the version of PROXY protocol we use for outgoing tcp connections except for FTP data connections.

**default**: not set, which means PROXY protocol won't be used

.. versionadded:: 1.11.3
