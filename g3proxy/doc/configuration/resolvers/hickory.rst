.. _configuration_resolver_hickory:

hickory
=======

.. versionadded:: 1.1.4

This is the resolver based on hickory dns library.

server
------

**required**, **type**: str | seq

Set the nameservers. All server will be tried before get a positive server response.

For *str* value, it may be one or more :ref:`ip addr str <conf_value_ip_addr_str>` joined with whitespace characters.

For *seq* value, each of its value should be :ref:`ip addr str <conf_value_ip_addr_str>`.

server_port
-----------

**optional**, **type**: u16

Set the port if the default port is not usable.

**default**: 53 for udp and tcp, 853 for dns-over-tls, 443 for dns-over-https

encryption
----------

**optional**, **type**: :ref:`dns encryption config <conf_value_dns_encryption_config>`

Set the encryption config.

**default**: not set

connect_timeout
---------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Specify the TCP/TLS/QUIC connect timeout value when connecting to the target server.

**default**: 10s

.. versionadded:: 1.7.37

request_timeout
---------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Specify response wait timeout value after a specific request has been to the target server.

**default**: 10s

.. versionadded:: 1.7.37

each_tries
----------

**optional**, **type**: i32

The number of tries for one specific target server if no valid responses received from previous connection.

.. note:: negative response is also considered valid

**default**: 2

.. versionchanged:: 1.7.37 this only control retries to a specific target server

each_timeout
------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Specify the timeout for waiting all responses from one specific target server.

**default**: 5s

retry_interval
--------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set retry interval between different target servers.

We will always receive responses from previous tried servers, and the first positive one will be used.

**default**: 1s

.. versionadded:: 1.7.37

bind_ip
-------

**optional**, **type**: :ref:`ip addr str <conf_value_ip_addr_str>`

Set the bind ip for the resolver while setting up sockets.

positive_min_ttl
----------------

**optional**, **type**: u32

Minimum TTL for positive responses.

**default**: 30

positive_max_ttl
----------------

**optional**, **type**: u32

Maximum TTL for positive responses. It should be longer than *positive_min_ttl*.

**default**: 3600

negative_min_ttl
----------------

**optional**, **type**: u32

Minimum TTL for negative responses.

**default**: 30, **alias**: negative_ttl
