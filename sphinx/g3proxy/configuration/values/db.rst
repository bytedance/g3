.. _configure_db_value_types:

**
DB
**

.. _conf_value_db_redis:

redis
=====

**yaml type**: map

Set the redis database address and connection params.

The following fields are supported:

* addr

  **required**, **type**: :ref:`upstream str <conf_value_upstream_str>`

  Set the address of the redis instance. The default port is 6379 which can be omitted.

* tls_client

  **optional**, **type**: :ref:`rustls client config <conf_value_rustls_client_config>`

  Enable tls and set the config.

  **default**: not set

  .. versionadded:: 1.9.7

* tls_name

  **optional**, **type**: :ref:`tls name <conf_value_tls_name>`

  Set the tls server name to verify peer certificate.

  **default**: not set

  .. versionadded:: 1.9.7

* db

  **optional**, **type**: int

  Set the database.

  **default**: 0

* username

  **optional**, **type**: str

  Set the username for redis 6 database if needed. It is required if connect to an ACL enabled redis 6 database.

  **default**: not set

* password

  **optional**, **type**: str

  Set the password.

  **default**: not set

* connect_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the connect timeout.

  **default**: 5s

* response_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the read timeout for redis command response.

  **default**: 2s, **alias**: read_timeout
