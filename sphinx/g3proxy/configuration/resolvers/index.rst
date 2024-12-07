.. _configuration_resolver:

********
Resolver
********

The type for each resolver config is *map*, with two always required keys:

* :ref:`name <conf_resolver_common_name>`, which specify the name of the resolver.
* :ref:`type <conf_resolver_common_type>`, which specify the real type of the resolver, decides how to parse other keys.

There are many types of resolver, each with a section below.

Resolvers
=========

.. toctree::
   :maxdepth: 1

   deny_all
   fail_over
   c_ares
   hickory

Common Keys
===========

This section describes the common keys, they may be used by many resolvers.

Most of them are the runtime (of the standalone resolver thread) config.

.. _conf_resolver_common_name:

name
----

**required**, **type**: :ref:`metrics name <conf_value_metrics_name>`

Set the name of the resolver.

.. _conf_resolver_common_type:

type
----

**required**, **type**: str

Set the type of the resolver.

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
