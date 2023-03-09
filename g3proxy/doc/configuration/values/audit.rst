
.. _configure_audit_value_types:

*****
Audit
*****

All audit value types are described here.

ICAP
====

.. _conf_value_audit_icap_service_config:

icap service config
-------------------

**type**: map | str

Config ICAP service.

For *str* value, the value will be treated as *url* as described following.

For *map* value, the keys are:

* url

  **required**, **type**: :ref:`url str <conf_value_url_str>`

  Set the ICAP service url.

* tcp_keepalive

  **optional**, **type**: :ref:`tcp keepalive <conf_value_tcp_keepalive>`

  Set the keep-alive config for the tcp connection to ICAP server.

  **default**: enabled with default value

* icap_connection_pool

  **optional**, **type**: :ref:`icap connection pool <conf_value_audit_icap_connection_pool>`

  Set the connection pool config.

  **default**: set with default value

* icap_max_header_size

  **optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

  Set the max header size when parsing response from the ICAP server.

  **default**: 8KiB

* preview_data_read_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout value for the read of preview data.
  If timeout, preview will not be used in the request send to the ICAP server.

  **default**: 4s

* respond_shared_names

  **optional**, **type**: :ref:`http header name <conf_value_http_header_name>` or seq of this

  Set the headers returned by ICAP server in REQMOD response that we should send in the following RESPMOD request.

  This config option now only apply to REQMOD service.

  **default**: not set

  .. versionadded:: 1.7.4

* bypass

  **optional**, **type**: bool

  Set if we should bypass if we can't connect to the ICAP server.

  **default**: false

.. _conf_value_audit_icap_connection_pool:

icap connection pool
--------------------

**type**: map

The keys are:

* check_interval

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the min idle check interval.
  New connections will be established if the idle connections are less than *min_idle_count*.

  **default**: 10s

* max_idle_count

  **optional*, **type**: usize

  Set the maximum idle connections count.

  **default**: 128

* min_idle_count

  **optional**, **type**: usize

  Set the minimum idle connections count.

  **default**: 16
