.. _protocol_helper_ip_locate:

=========
IP Locate
=========

We need a peer service in escaper :ref:`route_geoip <configuration_escaper_route_geoip>` config. This page describes the
protocol we used to communicate with the peer service.

The peer service should listen on a UDP port, which may be IPv4 or IPv6 based, we will sending requests to this port.

Each UDP packet from our side to the peer service will contains exactly one request. And each UDP packet from the peer
service should contains exactly one response.

The peer service can also push location and expire ttl responses directly to our side without any prior request.

Both the request and the response are structured data and should be encoded in `msgpack`_ format.

.. _msgpack: https://msgpack.org/

The root of the request and the response should be a map, the key may be a `key str` or a `key id`,
we will describe the keys of them in the following.

request
=======

ip
--

**required**, **id**: 1, **type**: string

Set the target IP address.

response
========

ip
--

**optional**, **id**: 1, **type**: string

The target ip address as specified in the request.

This should be present if it's a response to a request, or absent if it's a push response.

location
--------

**optional**, **id**: 2, **type**: :ref:`ip location <conf_value_ip_location>`

Set the IP location value.

ttl
---

**optional**, **id**: 3, **type**: u32

Set the expire ttl of the peer service.

If not set, the :ref:`default expire ttl <conf_value_ip_locate_service_default_expire_ttl>` config will
take effect.
