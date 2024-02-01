.. _configuration_resolver_c_ares:

c_ares
======

This is the resolver based on c-ares dns library.

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

**default**: 2000

.. versionchanged:: 1.7.27 change default value from 5000 to 2000 to match default values set in c-ares 1.20.1

each_tries
----------

**optional**, **type**: int

The number of tries the resolver will try contacting each name server before giving up.

**default**: 3

.. versionchanged:: 1.7.27 change default value from 2 to 3 to match default values set in c-ares 1.20.1

max_timeout
-----------

**optional**, **type**: int, **unit**: ms

The upper bound for timeout between sequential retry attempts. When retrying queries, the timeout is increased
from the requested timeout parameter, this caps the value.

**notes**: This will only have effect if link or build with c-ares 1.22.

**default**: 0, which is not explicitly set

.. versionadded:: 1.7.35

udp_max_quires
--------------

**optional**, **type**: int

The maximum number of udp queries that can be sent on a single ephemeral port to a given DNS server before a new
ephemeral port is assigned.

**notes**: This will only have effect if link or build with c-ares 1.20.

**default**: 0, which is unlimited

.. versionadded:: 1.7.35

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
