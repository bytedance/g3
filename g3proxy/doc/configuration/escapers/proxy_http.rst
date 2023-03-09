.. _configuration_escaper_proxy_http:

proxy_http
==========

This escaper will access the target upstream through another http proxy.

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
* :ref:`tcp_misc_opts <conf_escaper_common_tcp_misc_opts>`
* :ref:`pass_proxy_userid <conf_escaper_common_pass_proxy_userid>`
* :ref:`use_proxy_protocol <conf_escaper_common_use_proxy_protocol>`
* :ref:`peer negotiation timeout <conf_escaper_common_peer_negotiation_timeout>`
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

The key for rendezvous/jump hash is *<client-ip>[-<username>]-<upstream-host>*.

**default**: random

proxy_username
--------------

**optional**, **type**: :ref:`username <conf_value_username>`

Set the proxy username. The Basic auth scheme is used by default.

.. note::

  Conflict with :ref:`pass_proxy_userid <conf_escaper_common_pass_proxy_userid>`

proxy_password
--------------

**optional**, **type**: :ref:`password <conf_value_password>`

Set the proxy password. Required if username is present.

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

http_forward_capability
-----------------------

**optional**, **type**: :ref:`http forward capability <conf_value_http_forward_capability>`

Set the http forward capability if the next proxy.

**default**: all capability disabled

http_connect_rsp_header_max_size
--------------------------------

**optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

Set the max header size for received CONNECT response.

**default**: 4KiB

tcp_keepalive
-------------

**optional**, **type**: :ref:`tcp keepalive <conf_value_tcp_keepalive>`

Set tcp keepalive.

The tcp keepalive set in user config won't be taken into account.

**default**: no keepalive set
