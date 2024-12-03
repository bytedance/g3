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

The root of the request and the response should be a map, the key may be a `key str` or a `key id`,
we will describe the keys of them in the following.

request
=======

host
----

**required**, **id**: 1, **type**: string

Set the hostname of the target tls server. May be a domain or an IP address.

service
-------

**optional**, **id**: 2, **type**: string | u8

Set the tls service type. It should be returned in response.

**default**: http

.. versionadded:: 1.9.0

usage
-----

**optional**, **id**: 4, **type**: string | u8

Set the tls certificate usage type. It should be returned in response.

**default**: tls_server

.. versionadded:: 1.9.1

cert
----

**optional**, **id**: 3, **type**: pem string or der binary

The real upstream leaf cert in PEM string format or DER binary format.

.. versionadded:: 1.9.0

response
========

host
----

**required**, **id**: 1, **type**: string

The hostname as specified in the request.

service
-------

**optional**, **id**: 2, **type**: string | u8

Set the tls service type. It should be the same value as in the request.

**default**: http

.. versionadded:: 1.9.0

usage
-----

**optional**, **id**: 6, **type**: string | u8

Set the tls certificate usage type. It should be the same value as in the request.

**default**: tls_server

.. versionadded:: 1.9.1

cert
----

**required**, **id**: 3, **type**: pem string

The generated fake certificate (chain) in PEM format.

key
---

**required**, **id**: 4, **type**: pem string or der binary

The generated fake private key in PEM string format or in DER binary format.

ttl
---

**optional**, **id**: 5, **type**: u32

Set the expire ttl of this response.

If 0, the :ref:`protective cache ttl <conf_value_dpi_tls_cert_agent_protective_cache_ttl>` config will
take effect

.. note:: expired records will be cached some more time before cleared, see
 :ref:`cache_vanish_wait <conf_value_dpi_tls_cert_agent_cache_vanish_wait>` for more info.

**default**: 0
