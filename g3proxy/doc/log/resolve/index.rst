.. _log_resolve:

***********
Resolve Log
***********

The resolve log contains only errors in resolvers.

Shared Keys
===========

resolver_type
-------------

**required**, **type**: enum string

The type of the resolver.

resolver_name
-------------

**required**, **type**: string

The name of the resolver.

Values:

* c-ares
* fail-over
* deny-all

query_type
----------

**required**, **type**: enum string

The query type.

Values:

* A
* AAAA

duration
--------

**required**, **type**: time duration string

The time spent for this query action.

rr_source
---------

**required**, **type**: enum string

The source of the result.

Values:

* cache

  The result is fetched from cache.

* query

  The result is returned by drivers with real query to remote server.

error_type
----------

**required**, **type**: enum string

The main error type.

See the definition of **ResolverError** in *lib/g3-resolver/src/error.rs*.

error_subtype
-------------

**required**, **type**: enum string

The minor error type.

It's value is depends on the value of **error_type**.

See the definition of **ResolverError** in *lib/g3-resolver/src/error.rs*.

domain
------

**required**, **type**: domain string

The domain to query.

Sub Types
=========

.. toctree::
   :maxdepth: 2

   c_ares
   fail_over
   deny_all
