.. _configuration_resolver_c_ares:

c_ares
======

This is the resolver based on c-ares library.

server
------

**required**, **type**: str | seq

Set the nameservers.

For *str* value, it may be one or more :ref:`sockaddr str <conf_value_sockaddr_str>` joined with whitespace characters.

For *seq* value, each of its value should be :ref:`sockaddr str <conf_value_sockaddr_str>`.

The default port *53* will be used, if not port is specified in the value string.

Servers in different address families can be set in together.

each_timeout
------------

**optional**, **type**: int, **unit**: ms

The number of milliseconds each name server is given to respond to a query on the first try.
After the first try, the timeout algorithm becomes more complicated, but scales linearly with the value of timeout.

**default**: 5000

each_tries
----------

**optional**, **type**: int

The number of tries the resolver will try contacting each name server before giving up.

**default**: 2

round_robin
-----------

**optional**, **type**: bool

If true, perform round-robin selection of the nameservers configured for the channel for each resolution.

**default**: false

socket_send_buffer_size
-----------------------

**optional**, **type**: u32

Set the send buffer size for the socket.

**default**: not set, which should be the value of /proc/sys/net/core/wmem_default

socket_recv_buffer_size
-----------------------

**optional**, **type**: u32

Set the recv buffer size for the socket.

**default**: not set, which should be the value of /proc/sys/net/core/rmem_default

bind_ipv4
---------

**optional**, **type**: :ref:`ipv4 addr str <conf_value_ipv4_addr_str>`

Set the IPv4 bind ip for the resolver while setting up sockets.

bind_ipv6
---------

**optional**, **type**: :ref:`ipv6 addr str <conf_value_ipv6_addr_str>`

Set the IPv6 bind ip for the resolver while setting up sockets.

negative_ttl
------------

**optional**, **type**: u32

Time-to-Live (TTL) for negative caching of failed DNS lookups.
This also sets the lower cache limit on positive lookups.

**default**: 30

positive_ttl
------------

**optional**, **type**: u32

Upper limit on how long we will cache positive DNS responses. It should long than *negative_ttl*.

**default**: 3600
