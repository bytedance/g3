.. _configure_geoip_value_types:

*****
GeoIP
*****

.. _conf_value_iso_country_code:

iso country code
================

**yaml value**: str

The string should be ISO 3166 Alpha2 or Alpha3 code string.

.. _conf_value_continent_code:

continent code
==============

**yaml value**: str

The string should be:

  - AF, for Africa
  - AN, for Antarctica
  - AS, for Asia
  - EU, for Europe
  - NA, for North America
  - OC, for Oceania
  - SA, for South America

.. _conf_value_ip_location:

ip location
===========

**type**: map

Set the IP location info.

The keys are:

* network

  **required**, **type**: :ref:`ip network str <conf_value_ip_network_str>`

  Set the registered network address.

* country

  **optional**, **type**: :ref:`iso country code <conf_value_iso_country_code>`

  Set the country.

  **default**: not set

* continent

  **optional**, **type**: :ref:`continent code <conf_value_continent_code>`

  Set the continent.

  **default**: not set

* as_number

  **optional**, **type**: u32

  Set the AS Number.

  **default**: not set

* isp_name

  **optional**, **type**: str

  Set the name of it's ISP.

  **default**: not set

* isp_domain

  **optional**, **type**: str

  Set the domain of it's ISP.

  **default**: not set

.. versionadded:: 1.9.1

.. _conf_value_ip_locate_service:

ip locate service
=================

**type**: map | str

Set the config for the ip locate service.

The keys are:

* query_peer_addr

  **optional**, **type**: :ref:`env sockaddr str <conf_value_env_sockaddr_str>`

  Set the peer udp socket address.

  **default**: 127.0.0.1:2888

* query_socket_buffer

  **optional**, **type**: :ref:`socket buffer config <conf_value_socket_buffer_config>`

  Set the socket buffer config for the socket to peer.

  **default**: not set

* query_wait_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout for the cache runtime to wait response from the query runtime.

  **default**: 1s

.. _conf_value_ip_locate_service_default_expire_ttl:

* default_expire_ttl

  **optional**, **type**: u32

  Set the default expire ttl for the response.

  **default**: 10

* maximum_expire_ttl

  **optional**, **type**: u32

  Set the maximum expire ttl for the response.

  **default**: 300

* cache_request_batch_count

  **optional**, **type**: usize

  Set the batch request count in cache runtime.

  **default**: 10

* cache_request_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the request timeout for the caller.

  **default**: 2s

For *str* value, it will parsed as *query_peer_addr* and use default value for other fields.

.. versionadded:: 1.9.1
