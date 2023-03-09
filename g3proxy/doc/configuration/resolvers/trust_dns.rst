.. _configuration_resolver_trust_dns:

trust_dns
=========

.. versionadded:: 1.1.4

This is the resolver based on trust-dns library.

server
------

**required**, **type**: str | seq

Set the nameservers.

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

each_timeout
------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Specify the timeout for a request.

**default**: 5s

retry_attempts
--------------

**optional**, **type**: usize

Number of retries after lookup failure before giving up.

**default**: 2

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

Maximum TTL for positive responses.

**default**: 3600

negative_min_ttl
----------------

**optional**, **type**: u32

Minimum TTL for negative responses.

**default**: 30

negative_max_ttl
----------------

**optional**, **type**: u32

Maximum TTL for negative responses.

**default**: 3600
