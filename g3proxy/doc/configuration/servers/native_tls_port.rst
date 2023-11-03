.. _configuration_server_native_tls_port:

native_tls_port
===============

.. versionadded:: 1.7.29

This server provides plain tls port, which can be placed in front of other servers.

The following common keys are supported:

* :ref:`listen_in_worker <conf_server_common_listen_in_worker>`
* :ref:`ingress_network_filter <conf_server_common_ingress_network_filter>`

listen
------

**required**, **type**: :ref:`tcp listen <conf_value_tcp_listen>`

Set the listen config for this server.

The instance count setting will be ignored if *listen_in_worker* is correctly enabled.

tls_server
----------

**required**, **type**: :ref:`openssl server config <conf_value_openssl_server_config>`

Enable TLS on the listening socket by using OpenSSL and set TLS parameters.

server
------

**required**, **type**: str

Set name of the next server to send the accepted connections to.

The next server should be able to accept tls connections.

proxy_protocol
--------------

**optional**, **type**: :ref:`proxy protocol version <conf_value_proxy_protocol_version>`

Set the version of PROXY protocol we use for incoming tcp connections.

If set, connections with no matched PROXY Protocol message will be dropped.

The TLS handshake with the client will happen after we receive the PROXY Protocol message.

.. note:: The *ingress_network_filter* config option of this server will always applies to the real socket client address.

**default**: not set, which means PROXY protocol won't be used

proxy_protocol_read_timeout
---------------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the timeout value before we read a complete PROXY Protocol message.

**default**: 5s
