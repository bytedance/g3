.. _configuration_escaper_divert_tcp:

divert_tcp
==========

This escaper will redirect all streams to a next proxy server by sending a PROXY Protocol V2 message first.

The PPv2 Type-Values are:

* 0xE0 | Upstream Address

  The target upstream address, encoded in UTF-8 without trailing '\0'.
  This will always be set. And the next proxy server should connect to this upstream address.

* 0xE1 | TLS Verify Name

  The TLS verify name, encoded in UTF-8 without trailing '\0'.
  This will be set only if the TLS handshake is started on our side.

* 0xE2 | Username

  The username of the client, encoded in UTF-8 without trailing '\0'.
  This will be set only if client auth is enabled on our side.

* 0xE3 | Task ID

  The task id in UUID binary format. This will always be set.

The following interfaces are supported:

* tcp connect
* http(s) forward

There is no path selection support for this escaper.

The following common keys are supported:

* :ref:`shared_logger <conf_escaper_common_shared_logger>`
* :ref:`resolver <conf_escaper_common_resolver>`, **required** only if *proxy_addr* is domain
* :ref:`resolve_strategy <conf_escaper_common_resolve_strategy>`
* :ref:`tcp_sock_speed_limit <conf_escaper_common_tcp_sock_speed_limit>`
* :ref:`no_ipv4 <conf_escaper_common_no_ipv4>`
* :ref:`no_ipv6 <conf_escaper_common_no_ipv6>`
* :ref:`tcp_connect <conf_escaper_common_tcp_connect>`
* :ref:`happy eyeballs <conf_escaper_common_happy_eyeballs>`
* :ref:`tcp_misc_opts <conf_escaper_common_tcp_misc_opts>`
* :ref:`extra_metrics_tags <conf_escaper_common_extra_metrics_tags>`

proxy_addr
----------

**required**, **type**: :ref:`upstream str <conf_value_upstream_str>` | seq

Set the target proxy address. The default port is 3128 which can be omitted.

For *seq* value, each of its element must be :ref:`weighted upstream addr <conf_value_weighted_upstream_addr>`.

proxy_addr_pick_policy
----------------------

**optional**, **type**: :ref:`selective pick policy <conf_value_selective_pick_policy>`

Set the policy to select next proxy address.

The key for ketama/rendezvous/jump hash is *<client-ip>[-<username>]-<upstream-host>*.

**default**: random

bind_ipv4
---------

**optional**, **type**: :ref:`ipv4 addr str <conf_value_ipv4_addr_str>`

Set the bind ip address for inet sockets.

**default**: not set

bind_ipv6
---------

**optional**, **type**: :ref:`ipv6 addr str <conf_value_ipv6_addr_str>`

Set the bind ip address for inet6 sockets.

**default**: not set

tcp_keepalive
-------------

**optional**, **type**: :ref:`tcp keepalive <conf_value_tcp_keepalive>`

Set tcp keepalive.

The tcp keepalive set in user config won't be taken into account.

**default**: no keepalive set
