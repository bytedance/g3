.. _configuration_server_plain_quic_port:

plain_quic_port
===============

.. versionadded:: 1.7.30

This server provides plain quic port, which can be placed in front of other servers.

The following common keys are supported:

* :ref:`listen_in_worker <conf_server_common_listen_in_worker>`
* :ref:`ingress_network_filter <conf_server_common_ingress_network_filter>`

listen
------

**required**, **type**: :ref:`udp listen <conf_value_udp_listen>`

Set the udp listen config for this server.

The instance count setting will be ignored if *listen_in_worker* is correctly enabled.

quic_server
-----------

**required**, **type**: :ref:`rustls server config <conf_value_rustls_server_config>`

Set the crypto config for this quic server.

offline_rebind_port
-------------------

**optional**, **type**: u16

Set a rebind port if you want graceful shutdown.

The new port should be reachable from the client or it won't work as expected.

**default**: not set

server
------

**required**, **type**: str

Set name of the next server to send the accepted connections to.

The next server should be able to accept tcp connections.
