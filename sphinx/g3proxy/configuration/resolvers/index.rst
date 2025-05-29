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

**required**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

Set the name of the resolver.

.. _conf_resolver_common_type:

type
----

**required**, **type**: str

Set the type of the resolver.

.. _conf_resolver_common_graceful_stop_wait:

graceful_stop_wait
------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the wait duration before really shutdown the resolver thread. This applies to the cache runtime.

There may be queries running inside the resolver,
we don't wait all of them to finish but instead wait for a fixed time interval.

**default**: 30s

.. _conf_resolver_common_protective_query_timeout:

protective_query_timeout
------------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the query timeout value for queries sent to driver. This applies to the cache runtime.

The value should be larger than the value set in the driver specific timeout config.

**default**: 60s

.. _conf_resolver_common_positive_min_ttl:

positive_min_ttl
----------------

**optional**, **type**: u32

Minimum TTL for positive responses. This applies to the resolve driver.

**default**: 30

.. _conf_resolver_common_positive_max_ttl:

positive_max_ttl
----------------

**optional**, **type**: u32

Maximum TTL for positive responses. It should be longer than *positive_min_ttl*. This applies to the resolve driver.

**default**: 3600

.. _conf_resolver_common_negative_min_ttl:

negative_min_ttl
----------------

**optional**, **type**: u32

Minimum TTL for negative responses. This applies to the resolve driver.

**default**: 30, **alias**: negative_ttl

TTL Calculation
===============

A position record will be cached after fetched from driver, two TTL values will be used in the cache runtime:

* expire_ttl

  The record in the cache will be used if it can be found in the cache.

  But if it reaches the expire ttl, a new query will be made immediately when a new request received.

* vanish_ttl

  The record will be removed from the cache.

Here is the logic to calculate the values:

.. code-block:: shell

  if [ $RECORD_TTL -gt $(($POSITIVE_MAX_TTL + $POSITIVE_MIN_TTL)) ]
  then
    EXPIRE_TTL=$POSITIVE_MAX_TTL
    VANISH_TTL=$RECORD_TTL
  elif [ $RECORD_TTL -gt $(($POSITIVE_MIN_TTL + $POSITIVE_MIN_TTL)) ]
  then
    EXPIRE_TTL=$(($RECORD_TTL - $POSITIVE_MIN_TTL))
    VANISH_TTL=$RECORD_TTL
  elif [ $RECORD_TTL -gt $POSITIVE_MIN_TTL ]
  then
    EXPIRE_TTL=$POSITIVE_MIN_TTL
    VANISH_TTL=$RECORD_TTL
  else
    EXPIRE_TTL=$POSITIVE_MIN_TTL
    VANISH_TTL=$(($POSITIVE_MIN_TTL + 1))
  fi
