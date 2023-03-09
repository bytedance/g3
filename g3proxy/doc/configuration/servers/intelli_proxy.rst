.. _configuration_server_intelli_proxy:

intelli_proxy
=============

Intelligent Proxy port, it will do protocol detection and then send to other servers if detected.

The following common keys are supported:

* :ref:`listen <conf_server_common_listen>`
* :ref:`listen_in_worker <conf_server_common_listen_in_worker>`
* :ref:`ingress_network_filter <conf_server_common_ingress_network_filter>`

http_server
-----------

**required**, **type**: str

Set name of the next http_proxy server to send the accepted connections to.

socks_server
------------

**required**, **type**: str

Set name of the next socks_proxy server to send the accepted connections to.

protocol_detection_channel_size
-------------------------------

**optional**, **type**: usize

The connection will be send to a task channel after it's protocol is detected. This config option set the channel size.

If the channel is full, the connection will be closed silently.

**default**: 4096

protocol_detection_timeout
--------------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the timeout duration that we wait to check the protocol of each connection.

If timeout, the connection will be closed silently.

**default**: 4s

protocol_detection_max_jobs
---------------------------

**optional**, **type**: usize

Set the limit of protocol detection jobs.

If the limit is reached, the connection will be closed silently.

**default**: 4096
