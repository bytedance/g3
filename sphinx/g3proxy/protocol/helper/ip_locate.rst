.. _protocol_helper_ip_locate:

=========
IP Locate
=========

We need a peer service in escaper :ref:`route_geoip <configuration_escaper_route_geoip>` config. This page describes the
protocol we used to communicate with the peer service.

The peer service should listen on a UDP port, which may be IPv4 or IPv6 based, we will sending requests to this port.

Each UDP packet from our side to the peer service will contains exactly one request. And each UDP packet from the peer
service should contains exactly one response.

The peer service can also push location response directly to our side without any prior request.

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

ttl
---

**optional**, **id**: 2, **type**: u32

Set the expire ttl of the response.

If not set, the :ref:`default expire ttl <conf_value_ip_locate_service_default_expire_ttl>` config will
take effect.

network
-------

**required**, **id**: 3, **type**: :ref:`ip network str <conf_value_ip_network_str>`

Set the registered network address.

country
-------

**optional**, **id**: 4, **type**: :ref:`iso country code <conf_value_iso_country_code>`

Set the country.

continent
---------

**optional**, **id**: 5, **type**: :ref:`continent code <conf_value_continent_code>`

Set the continent

as_number
---------

**optional**, **id**: 6, **type**: u32

Set the AS Number.

isp_name
--------

**optional**, **id**: 7, **type**: str

Set the name of it's ISP.

isp_domain
----------

**optional**, **id**: 8, **type**: str

Set the domain of it's ISP.
