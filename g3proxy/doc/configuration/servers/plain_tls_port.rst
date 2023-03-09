.. _configuration_server_plain_tls_port:

plain_tls_port
==============

This server provides plain tls port, which can be placed in front of other servers.

The following common keys are supported:

* :ref:`listen <conf_server_common_listen>`
* :ref:`listen_in_worker <conf_server_common_listen_in_worker>`
* :ref:`ingress_network_filter <conf_server_common_ingress_network_filter>`
* :ref:`tls_server <conf_server_common_tls_server>`

  This is required for this server.

server
------

**required**, **type**: str

Set name of the next server to send the accepted connections to.

The next server should be able to accept tls connections.
