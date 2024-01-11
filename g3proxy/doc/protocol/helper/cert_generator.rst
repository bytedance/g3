.. _protocol_helper_cert_generator:

=====================
Certificate Generator
=====================

We need a peer service in auditor :ref:`tls cert agent <conf_auditor_tls_cert_agent>` config. This page describes the
protocol we used to communicate with the peer service.

The peer service should listen on a UDP port, which may be IPv4 or IPv6 based, we will sending requests to this port.

Each UDP packet from our side to the peer service will contains exactly one request. And each UDP packet from the peer
service should contains exactly one response.

Both the request and the response are structured data and should be encoded in `msgpack`_ format.

.. _msgpack: https://msgpack.org/

The root of the request and the response should be a map, we will describe the keys of them in the following.

request
=======

host
----

**required**, **type**: string

Set the hostname of the target tls server. May be a domain or an IP address.

response
========

host
----

**required**, **type**: string

The hostname as specified in the request.

cert
----

**required**, **type**: string

The generated fake certificate in PEM format.

key
---

**required**, **type**: string

The generated fake private key in PEM format.

ttl
---

**optional**, **type**: u32

Set the expire ttl of this response.

If 0, the :ref:`protective cache ttl <conf_value_dpi_tls_cert_agent_protective_cache_ttl>` config will
take effect

.. note:: expired records will be cached some more time before cleared, see
 :ref:`cache_vanish_wait <conf_value_dpi_tls_cert_agent_cache_vanish_wait>` for more info.

**default**: 0
