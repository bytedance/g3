.. _configuration_escaper_direct_float:

************
direct_float
************

This escaper will access the target upstream from local machine directly. The local bind ip, which is required,
can be set via the `publish` rpc method.

The following interfaces are supported:

* tcp connect
* http(s) forward

The Cap'n Proto RPC publish command is supported on this escaper, the published data should be a map, with the keys:

* ipv4

  Set the IPv4 bind ip address(es).
  The value could be an array of or just one :ref:`bind ip <config_escaper_dynamic_bind_ip>`.

* ipv6

  Set the IPv6 bind ip address(es).
  The value could be an array of or just one :ref:`bind ip <config_escaper_dynamic_bind_ip>`.

There is no path selection support for this escaper.

Config Keys
===========

The following common keys are supported:

* :ref:`shared_logger <conf_escaper_common_shared_logger>`
* :ref:`resolver <conf_escaper_common_resolver>`, **required**
* :ref:`resolve_strategy <conf_escaper_common_resolve_strategy>`

  The user custom resolve strategy will be taken into account.

* :ref:`tcp_sock_speed_limit <conf_escaper_common_tcp_sock_speed_limit>`
* :ref:`udp_sock_speed_limit <conf_escaper_common_udp_sock_speed_limit>`
* :ref:`no_ipv4 <conf_escaper_common_no_ipv4>`
* :ref:`no_ipv6 <conf_escaper_common_no_ipv6>`
* :ref:`tcp_connect <conf_escaper_common_tcp_connect>`

  The user tcp connect params will be taken into account.

* :ref:`tcp_misc_opts <conf_escaper_common_tcp_misc_opts>`
* :ref:`extra_metrics_tags <conf_escaper_common_extra_metrics_tags>`

cache_ipv4
----------

**recommend**, **type**: :ref:`file path <conf_value_file_path>`

Set the cache file for published IPv4 IP Address(es).

It is recommended to set this as the fetch of peers at startup may be finished after the first batch of requests.

The file will be created if not existed.

**default**: not set

cache_ipv6
----------

**recommend**, **type**: :ref:`file path <conf_value_file_path>`

Set the cache file for published IPv6 IP Address(es).

It is recommended to set this as the fetch of peers at startup may be finished after the first batch of requests.

The file will be created if not existed.

**default**: not set

egress_network_filter
---------------------

**optional**, **type**: :ref:`egress network acl rule <conf_value_egress_network_acl_rule>`

Set the network filter for the (resolved) remote ip address.

**default**: all permitted except for loopback and link-local addresses

happy_eyeballs
--------------

**optional**, **type**: :ref:`happy eyeballs <conf_value_happy_eyeballs>`

Set the HappyEyeballs config.

**default**: default HappyEyeballs config

.. versionadded:: 1.5.3

tcp_keepalive
-------------

**optional**, **type**: :ref:`tcp keepalive <conf_value_tcp_keepalive>`

Set tcp keepalive.

The tcp keepalive set in user config will be taken into account.

**default**: 60s

resolve_redirection
-------------------

**optional**, **type**: :ref:`resolve redirection <conf_value_resolve_redirection>`

Set the dns redirection rules at escaper level.

**default**: not set

.. _config_escaper_dynamic_bind_ip:

Bind IP
=======

We use json string to represent a dynamic bind ip, with a map type as root element.

* ip

  **required**, **type**: :ref:`ip addr str <conf_value_ip_addr_str>`

  Set the IP address. The address family should match the type of the publish key described above.

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

  Set the expire time of this dynamic ip.

  **default**: not set

If all optional fields can be set with the default value, the root element can be just a *ip*.
