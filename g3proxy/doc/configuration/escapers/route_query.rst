.. _configuration_escaper_route_query:

route_query
===========

This escaper allows to select a next escaper based on query to another service through a UDP socket.

There is no path selection support for this escaper.

No common keys are supported.

.. _configuration_escaper_route_query_fallback_node:

fallback_node
-------------

**required**, **type**: string

Set the fallback escaper name.

query_allowed_next
------------------

**required**, **type**: seq

Set all the next escapers those are allowed to use in the query result. Each element should be the next escaper name.
If the selected escaper name is not found in this list, the fallback escaper will be used.

.. _configuration_escaper_route_query_pass_client_ip:

query_pass_client_ip
--------------------

**optional**, **type**: bool

Set whether we should also send client_ip in the query message.

**default**: false

cache_request_batch_count
-------------------------

**optional**, **type**: usize

Set how many consequent query requests we should handle in the cache runtime before yield out to the next loop.

**default**: 10

cache_request_timeout
---------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set how many time we should spend on waiting responses from cache runtime after sending query request.

The fallback node will be used if timeout occur.

**default**: 100ms

cache_pick_policy
-----------------

**optional**, **type**: :ref:`selective pick policy <conf_value_selective_pick_policy>`

Set the policy to select next proxy address from the query result.

The key for rendezvous/jump hash is *<client-ip>*.

**default**: rendezvous

query_peer_addr
---------------

**optional**, **type**: :ref:`sockaddr str <conf_value_sockaddr_str>`

Set the socket address of the service that we should send queries to.

**default**: 127.0.0.1:1053

query_socket_buffer
-------------------

**optional**, **type**: :ref:`socket buffer config <conf_value_socket_buffer_config>`

Set the socket buffer config for the UDP socket we will use.

**default**: not set

query_wait_timeout
------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set how many time we should wait for response from the peer service.

Empty reply will be send back to cache runtime if timeout occur.

**default**: 10s

.. _configuration_escaper_route_query_protective_cache_ttl:

protective_cache_ttl
--------------------

**optional**, **type**: usize

Set the cache ttl for failed or zero-ttl query results.

**default**: 10

maximum_cache_ttl
-----------------

**optional**, **type**: usize

Set the maximum cache ttl for query results.

**default**: 1800

.. _configuration_escaper_route_query_vanish_after_expired:

cache_vanish_wait
-----------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Clean the record from the cache if it has been expired such many time.

We still cache expired records some time before clean them as a new query will spend more time and the new query result
will have a big chance to be the same with the expired one.

**default**: 30s, **alias**: vanish_after_expire
