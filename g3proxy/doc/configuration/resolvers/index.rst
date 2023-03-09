.. _configuration_resolver:

********
Resolver
********

The type for each resolver config is *map*, with two always required keys:

* *name*, which specify the name of the resolver.
* *type*, which specify the real type of the resolver, decides how to parse other keys.

There are many types of resolver, each with a section below.

Resolvers
=========

.. toctree::
   :maxdepth: 2

   deny_all
   fail_over
   c_ares
   trust_dns

Common Keys
===========

This section describes the common keys, they may be used by many resolvers.

Most of them are the runtime (of the standalone resolver thread) config.

graceful_stop_wait
------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the wait duration before really shutdown the resolver thread.

There may be queries running inside the resolver,
we don't wait all of them to finish but instead wait for a fixed time interval.

**default**: 30s

protective_query_timeout
------------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the query timeout value for queries sent to driver.

The value should be larger than the value set in the driver specific timeout config.

**default**: 60s
