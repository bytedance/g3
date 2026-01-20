.. _configuration_auth_user_group_ldap:

LDAP
====

The user group that auth user with remote a LDAP server (simple bind).

The following common keys are supported:

* :ref:`name <conf_auth_user_group_name>`
* :ref:`type <conf_auth_user_group_type>`
* :ref:`static users <conf_auth_user_group_static_users>`
* :ref:`source <conf_auth_user_group_source>`
* :ref:`cache <conf_auth_user_group_cache>`
* :ref:`refresh_interval <conf_auth_user_group_refresh_interval>`
* :ref:`anonymous_user <conf_auth_user_group_anonymous_user>`

ldap_url
--------

**required**, **type**: LDAP URL

Set the LDAP url in format `<schema>://<server_name>:[<port>]/<base_dn>`.
The schema should be one of `ldap` or `ldaps`, the default for `ldap` is 389 while 636 will be used for `ldaps`.

tls_client
----------

**optional**, **type**: :ref:`openssl tls client config <conf_value_openssl_tls_client_config>`

Set TLS parameters for this local TLS client.
If set to empty map, a default config is used.

If the schema of LDAP url is "ldap" and this has been set, then "STARTTLS" will be used.

If the schema is "ldaps", a default value will be used if not set.

**default**: not set

unmanaged_user
--------------

**optional**, **type**: :ref:`user <configuration_auth_user>`

Set and enable unmanaged users.

This is a template user config for all users that auth OK with the LDAP server but not has been set
in both static and dynamic users config.

If not set, only static or dynamic users will be allowed.

**default**: not set

max_message_size
----------------

**optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

Set the max header size when parsing response from the LDAP server.

**default**: 256

connect_timeout
---------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the timeout value when TCP connect to the LDAP server.

**default**: 4s

response_timeout
----------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the timeout value for the read of response from LDAP server.

**default**: 2s

connection_pool
---------------

**optional**, **type**: :ref:`connection pool <conf_value_connection_pool_config>`

Set the connection pool config.

**default**: set with default value

queue_channel_size
------------------

**optional**, **type**: usize

Set the queue channel size value when auth with the LDAP server for a client request.

**default**: 64

queue_wait_timeout
------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the timeout value when auth with the LDAP server for a client request.

**default**: 4s
