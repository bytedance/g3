.. _protocol_helper_route_query:

===========
Route Query
===========

We need a peer service in :ref:`route_query escaper <configuration_escaper_route_query>`. This page describes the
protocol we used to communicate with the peer service.

The peer service should listen on a UDP port, which may be IPv4 or IPv6 based, we will sending requests to this port.

Each UDP packet from our side to the peer service will contains exactly one request. And each UDP packet from the peer
service should contains exactly one response.

Both the request and the response are structured data and should be encoded in `msgpack`_ format.

.. _msgpack: https://msgpack.org/

The root of the request and the response should be a map, we will describe the keys of them in the following.

request
=======

id
--

**required**, **type**: uuid binary

Set the id of the request.

user
----

**required**, **type**: string

Set the username of the proxy request. The value may be an empty string if auth is disabled on the proxy side.

host
----

**required**, **type**: string

Set the target host of the proxy request. May be a domain or an IP address.

client_ip
---------

Set the client ip address. This will be set only of :ref:`query_pass_client_ip <configuration_escaper_route_query_pass_client_ip>` is enabled.

response
========

id
--

**required**, **type**: uuid binary | uuid string

Set the id of the corresponding request.

nodes
-----

**optional**, **type**: string | seq

Set the next escaper(s) those can be selected.

For *seq* value, each of its element must be :ref:`weighted name str <conf_value_weighted_name_str>`.

If empty, the :ref:`fallback node <configuration_escaper_route_query_fallback_node>` escaper config will take effect.

**default**: empty

ttl
---

**optional**, **type**: u32

Set the expire ttl of this response.

If 0, the :ref:`protective cache ttl <configuration_escaper_route_query_protective_cache_ttl>` escaper config will
take effect

.. note:: expired records will be cached some more time before cleared, see
 :ref:`vanish_after_expired <configuration_escaper_route_query_vanish_after_expired>` for more info.

**default**: 0
