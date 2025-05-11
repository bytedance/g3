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

bind_interface
--------------

**optional**, **type**: :ref:`interface name <conf_value_interface_name>`

Bind the outgoing socket to a particular device like “eth0”.

.. note:: This is only supported on Linux based OS.

**default**: not set

.. versionadded:: 1.11.3

tcp_misc_opts
-------------

**optional**, **type**: :ref:`tcp misc sock opts <conf_value_tcp_misc_sock_opts>`

Set misc tcp socket options.

**default**: not set, nodelay is default enabled

.. versionadded:: 1.11.3

udp_misc_opts
-------------

**optional**, **type**: :ref:`udp misc sock opts <conf_value_udp_misc_sock_opts>`

Set misc udp socket options.

**default**: not set

.. versionadded:: 1.11.3

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

positive_del_ttl
----------------

**optional**, **type**: u32

The TTL to delete the positive record from trash.

The records in the trash will be reused if the driver failed to fetch new records.

The trashed records will be deleted if:

- the del_tel timeout reached
- new positive records fetched
- empty records fetched from server
- NotFound fetched from server

**default**: 7200

.. versionadded:: 1.11.6

negative_min_ttl
----------------

**optional**, **type**: u32

Minimum TTL for negative responses.

**default**: 30, **alias**: negative_ttl
