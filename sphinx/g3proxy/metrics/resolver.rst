.. _metrics_resolver:

################
Resolver Metrics
################

The resolver metrics just contain the query stats on the resolver.

The following are the tags for all resolver metrics:

* :ref:`daemon_group <metrics_tag_daemon_group>`
* :ref:`stat_id <metrics_tag_stat_id>`

* resolver

  Set the resolver name.

* rr_type

  Show the rr_type of the query, such as 'A' or 'AAAA'.

Query
=====

The metrics names are:

* resolver.query.total

  **type**: count

  Show the total queries to this resolver.

* resolver.query.cached

  **type**: count

  Show the total queries that has local cached result.

* resolver.query.driver.total

  **type**: count

  Show the total queries that trigger a direct query to dns server, a.k. the queries to the dns server.

* resolver.query.driver.timeout

  **type**: count

  Show the total queries sent to the dns server and timed out.

* resolver.query.driver.refused

  **type**: count

  Show the total queries sent to the the dns server and refused.

* resolver.query.driver.malformed

  **type**: count

  Show the total queries reported malformed by driver.

* resolver.query.server.refused

  **type**: count

  Show the total queries reported refused by dns server.

* resolver.query.server.malformed

  **type**: count

  Show the total queries reported malformed by dns server.

* resolver.query.server.not_found

  **type**: count

  Show the total queries reported not found by dns server.

* resolver.query.server.serv_fail

  **type**: count

  Show the total queries reported server fail by dns server.

Memory
======

The metric names are:

* resolver.memory.cache.capacity

  **type**: gauge

  Show the capacity of the result cache hash table.

* resolver.memory.cache.length

  **type**: gauge

  Show how many records in the result cache hash table.

* resolver.memory.doing.capacity

  **type**: gauge

  Show the capacity of the doing hash table (query has been sent without any results).

* resolver.memory.doing.length

  **type**: gauge

  Show how many records in the doing hash table (query has been sent without any results).
