.. _configuration_user_group_user:

****
User
****

The user config is in map format. We can specify how to authenticate the user, set limitations and we may also specify
som custom actions for each user.

name
----

**required**, **type**: :ref:`username <conf_value_username>`

Set the username.

token
-----

**required**, **type**: mix

Set the token used to authenticate the user. The token can be in the following types:

* str

  The value should be a string in unix format, see crypt(5).

* map

  The key *type* specify the real type.

  * fast_hash

    A custom type. We use salt, and one or more value of md5, sha1, blake3. The hash is weak, but fast.
    The values for *salt*, *md5*, *sha1*, *blake3* should be in hex encoded ascii string.

  * xcrypt_hash

    The required key is *value*, which value should be a valid crypt(5) string.

The currently supported crypt(5) methods are: md5, sha256, sha512.

expire
------

**optional**, **type**: :ref:`rfc3339 datetime str <conf_value_rfc3339_datetime_str>`

Set when the user should be considered expired. The check interval is set by
:ref:`refresh interval <conf_user_group_refresh_interval>` in group config.

**default**: not set

block_and_delay
---------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Block the user, and delay sending of the error response by the specified duration.

The response code for blocked user will be forbidden instead of auth failed.

**default**: not set

proxy_request_filter
--------------------

**optional**, **type**: :ref:`proxy request acl rule <conf_value_proxy_request_acl_rule>`

Set the proxy request types that we should handle.

**default**: not set

dst_host_filter_set
-------------------

**optional**, **type**: :ref:`dst host acl rule set <conf_value_dst_host_acl_rule_set>`

Set the filter for dst host of each request, which means it won't apply to udp associate tasks.

**default**: not set

dst_port_filter
---------------

**optional**, **type**: :ref:`exact port acl rule <conf_value_exact_port_acl_rule>`

Set the filter for dst port of each request, which means it won't apply to udp associate tasks.

**default**: not set

http_user_agent_filter
----------------------

**optional**, **type**: :ref:`user agent acl rule <conf_value_user_agent_acl_rule>`

Set the filter for HTTP User-Agent header.

.. note:: This only applies to layer-7 http traffic, including http forward and https forward.

**default**: not set

tcp_connect
-----------

**optional**, **type**: :ref:`tcp connect <conf_value_tcp_connect>`

Set user level tcp connect params, which will take effect for *direct* type escapers.
And this will be limited by the escaper level settings.

**default**: not set

tcp_sock_speed_limit
--------------------

**optional**, **type**: :ref:`tcp socket speed limit <conf_value_tcp_sock_speed_limit>`

Set speed limit for each tcp socket.

**default**: no limit, **alias**: tcp_conn_speed_limit | tcp_conn_limit

.. versionchanged:: 1.4.0 changed name to tcp_sock_speed_limit

udp_sock_speed_limit
---------------------

**optional**, **type**: :ref:`udp socket speed limit <conf_value_udp_sock_speed_limit>`

Set speed limit for each udp socket.

**default**: no limit, **alias**: udp_relay_speed_limit | udp_relay_limit

.. versionchanged:: 1.4.0 changed name to udp_sock_speed_limit

tcp_remote_keepalive
--------------------

**optional**, **type**: :ref:`tcp keepalive <conf_value_tcp_keepalive>`

Set tcp keepalive for the remote tcp socket.

The tcp keepalive set in user config will only be taken into account in Direct type escapers.

**default**: no keepalive set

tcp_remote_misc_opts
--------------------

**optional**, **type**: :ref:`tcp misc sock opts <conf_value_tcp_misc_sock_opts>`

Set misc tcp socket options for the remote tcp socket.

The user level TOS and Mark config will overwrite the one set at escaper level.
Other fields will be limited to the smaller ones.

**default**: not set

udp_remote_misc_opts
--------------------

**optional**, **type**: :ref:`udp misc sock opts <conf_value_udp_misc_sock_opts>`

Set misc udp socket options for the remote udp socket.

The user level TOS and Mark config will overwrite the one set at escaper level.
Other fields will be limited to the smaller ones.

**default**: not set

tcp_client_misc_opts
--------------------

**optional**, **type**: :ref:`tcp misc sock opts <conf_value_tcp_misc_sock_opts>`

Set misc tcp socket options for the client tcp socket before task connecting stage.

The user level TOS and Mark config will overwrite the one set at escaper level.
Other fields will be limited to the smaller ones.

**default**: not set

udp_client_misc_opts
--------------------

**optional**, **type**: :ref:`udp misc sock opts <conf_value_udp_misc_sock_opts>`

Set misc udp socket options for the client udp socket.

The user level TOS and Mark config will overwrite the one set at server level.
Other fields will be limited to the smaller ones.

**default**: not set

http_upstream_keepalive
-----------------------

**optional**, **type**: :ref:`http keepalive <conf_value_http_keepalive>`

Set http keepalive config at user level.

**default**: set with default value

tcp_conn_rate_limit
-------------------

**optional**, **type**: :ref:`rate limit quota <conf_value_rate_limit_quota>`

Set rate limit on client side new connections.

The same connection used for different users will be counted for each of them.

**default**: no limit, **alias**: tcp_conn_limit_quota

.. versionadded:: 1.4.0

request_rate_limit
------------------

**optional**, **type**: :ref:`rate limit quota <conf_value_rate_limit_quota>`

Set rate limit on request.

**default**: no limit, **alias**: request_limit_quota

request_max_alive
-----------------

**optional**, **type**: usize, **alias**: request_alive_max

Set max alive requests at user level.

Even if not set, the max alive requests should not be more than usize::MAX.

**default**: no limit

resolve_strategy
----------------

**optional**, **type**: :ref:`resolve strategy <conf_value_resolve_strategy>`

Set an user custom resolve strategy, within the range of the one set on the escaper.
Not all escapers support this, see the documentation for each escaper for more info.

**default**: not custom resolve strategy is set

resolve_redirection
-------------------

**optional**, **type**: :ref:`resolve redirection <conf_value_resolve_redirection>`

Set the dns redirection rules at user level.

**default**: not set

log_rate_limit
--------------

**optional**, **type**: :ref:`rate limit quota <conf_value_rate_limit_quota>`

Set rate limit on log request.

**default**: no limit, **alias**: log_limit_quota

.. _config_user_log_uri_max_chars:

log_uri_max_chars
-----------------

**optional**, **type**: usize

Set the max number of characters of uri should be logged in logs.

If set, this will override the one set in server level.

If not set, the one in server level will take effect.

The password in uri will be replaced by *xyz* before logging.

**default**: not set

task_idle_max_count
-------------------

**optional**, **type**: i32

The task will be closed if the idle check return IDLE the times as this value.

This will overwrite the one set at server side,
see :ref:`server task_idle_max_count <conf_server_common_task_idle_max_count>`.

The idle check interval can only set at server side,
see :ref:`server task_idle_check_duration <conf_server_common_task_idle_check_duration>`.

**default**: 1

socks_use_udp_associate
-----------------------

**optional**, **type**: bool

Set if we should use socks udp associate instead of the simplified udp connect method.

**default**: false

.. versionadded:: 1.3.0

audit
-----

**optional**, **type**: :ref:`user audit <configuration_user_group_user_audit>`

Set audit config for this user.

**default**: set with default values

.. versionadded:: 1.7.0

explicit_sites
--------------

**optional**, **type**: seq of :ref:`user site <configuration_user_group_user_site>`

Set explicit sites for this user.

.. versionadded:: 1.3.4
