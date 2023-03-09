.. _configuration_resolver_fail_over:

fail_over
=========

This is a virtual resolver designed to fail over between (real) resolvers.

Rules for result selection:

1. The **success** result of the primary resolver will always be used before the timeout.
2. The first **success** result either from the primary or the standby resolver will be used after the timeout.
3. If no success result, the last error one will be used.

primary
-------

**required**, **type**: string

Set the primary resolver to use.

standby
-------

**required**, **type**: string

Set the standby resolver to use.

timeout
-------

**optional**, **type**:

Set the timeout for primary lookup.

**default**: 100ms

negative_ttl
------------

**optional**, **type**: u32

Time-to-Live (TTL) for negative caching of failed DNS lookups.

**default**: 30
