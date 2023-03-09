.. _configuration_log_driver_fluentd:

fluentd
=======

.. versionadded:: 1.5.0

The fluentd driver config is is map format.

We can set it to send logs to fluentd / fluent-bit by using it's `Forward Protocol`_.

.. _Forward Protocol: https://github.com/fluent/fluentd/wiki/Forward-Protocol-Specification-v1

The tags in the fluentd event message will be g3proxy.Task / g3proxy.Escape / g3proxy.Resolve for the corresponding logs.

The keys are described below.

address
-------

**optional**, **type**: :ref:`sockaddr str <conf_value_sockaddr_str>`

Set the tcp address of the fluentd server.

**default**: 127.0.0.1:24224

bind_ip
-------

**optional**, **type**: :ref:`ip addr str <conf_value_ip_addr_str>`

Set the ip address to bind to for the local socket.

**default**: not set

shared_key
----------

**optional**, **type**: str

Set the shared key if authentication is required.

The handshake stage will be skipped if shared key is not set.

**default**: not set

username
--------

**optional**, **type**: str

Set the username if authorization is required.

This will only be used if authorization is required by the server.

**default**: not set

password
--------

**optional**, **type**: str

Set the password if authorization is required.

This will only be used if authorization is required by the server.

**default**: not set

hostname
--------

**optional**, **type**: str

Set a custom hostname.

**default**: local hostname

tcp_keepalive
-------------

**optional**, **type**: :ref:`tcp keepalive <conf_value_tcp_keepalive>`

Set the tcp keepalive config for the connection to fluentd server.

**default**: enabled with system default values

tls_client
----------

**optional**, **type**: :ref:`openssl tls client config <conf_value_openssl_tls_client_config>`

Enable tls and set the config.

**default**: not set

connect_timeout
---------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the timeout value for the connection to fluentd server, including tcp connect, tls handshake, fluentd handshake.

**default**: 10s

connect_delay
-------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the delay time if the connect to fluentd server failed. All messages received will be dropped during this stage.

**default**: 10s

write_timeout
-------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the write timeout for each message. The message will be dropped if timeout.

default: 1s

flush_interval
--------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the flush interval for the connection to fluentd server.

**default**: 100ms

retry_queue_len
---------------

**optional**, **type**: usize

Set how many events will be queued up to retry when connect or write failed.
Note the write timeout events will be dropped directly.

**default**: 10
