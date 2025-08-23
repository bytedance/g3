.. _configuration_auditor:

*******
Auditor
*******

The type for each auditor config is *map*, the keys are as follows:

name
----

**required**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

Set the auditor name, which will can be referenced in :ref:`server config <conf_server_common_auditor>`.

.. _conf_auditor_protocol_inspection:

protocol_inspection
-------------------

**optional**, **type**: :ref:`protocol inspection <conf_value_dpi_protocol_inspection>`

Set basic config for protocol inspection.

**default**: set with default value

server_tcp_portmap
------------------

**optional**, **type**: :ref:`server tcp portmap <conf_value_dpi_server_tcp_portmap>`

Set the portmap for protocol inspection based on server side tcp port.

**default**: set with default value

client_tcp_portmap
------------------

**optional**, **type**: :ref:`client tcp portmap <conf_value_dpi_client_tcp_portmap>`

Set the portmap for protocol inspection based on client side tcp port.

**default**: set with default value

.. _conf_auditor_tls_cert_agent:

tls_cert_agent
--------------

**optional**, **type**: :ref:`tls cert agent <conf_value_dpi_tls_cert_agent>`

Set certificate generator for TLS interception.

If not set, TLS interception will be disabled.

**default**: not set, **alias**: tls_cert_generator

tls_ticketer
------------

**optional**, **type**: :ref:`tls ticketer <conf_value_tls_ticketer>`

Set a (remote) rolling TLS ticketer.

**default**: not set

.. versionadded:: 1.9.9

.. _conf_auditor_tls_interception_client:

tls_interception_client
-----------------------

**optional**, **type**: :ref:`tls interception client <conf_value_dpi_tls_interception_client>`

Set the tls client config for server handshake in TLS interception.

**default**: set with default value

tls_interception_server
-----------------------

**optional**, **type**: :ref:`tls interception server <conf_value_dpi_tls_interception_server>`

Set the tls server config for client handshake in TLS interception.

**default**: set with default value

tls_stream_dump
---------------

**optional**, **type**: :ref:`stream dump <conf_value_dpi_stream_dump>`

Set this to dump the intercepted inner tls streams to a remote service.

**default**: not set

.. versionadded:: 1.7.34

log_uri_max_chars
-----------------

**optional**, **type**: usize

Set the max chars for the log of URI.

**default**: 1024

.. _conf_auditor_h1_interception:

h1_interception
---------------

**optional**, **type**: :ref:`h1 interception <conf_value_dpi_h1_interception>`

Set http 1.x interception config.

**default**: set with default value

h2_inspect_policy
-----------------

**optional**, **type**: :ref:`protocol inspect policy <conf_value_dpi_protocol_inspect_policy>`

Set what we should do with HTTP/2.0 traffic.

**default**: intercept

.. versionadded:: 1.9.0

.. _conf_auditor_h2_interception:

h2_interception
---------------

**optional**, **type**: :ref:`h2 interception <conf_value_dpi_h2_interception>`

Set http 2.0 interception config.

**default**: set with default value

websocket_inspect_policy
------------------------

**optional**, **type**: :ref:`protocol inspect policy <conf_value_dpi_protocol_inspect_policy>`

Set what we should do with WebSocket traffic.

**default**: intercept

.. versionadded:: 1.9.8

smtp_inspect_policy
-------------------

**optional**, **type**: :ref:`protocol inspect policy <conf_value_dpi_protocol_inspect_policy>`

Set what we should do with SMTP traffic.

**default**: intercept

.. versionadded:: 1.9.0

.. _conf_auditor_smtp_interception:

smtp_interception
-----------------

**optional**, **type**: :ref:`smtp interception <conf_value_dpi_smtp_interception>`

Set the SMTP Interception config options.

**default**: set with default value

.. versionadded:: 1.9.2

imap_inspect_policy
-------------------

**optional**, **type**: :ref:`protocol inspect policy <conf_value_dpi_protocol_inspect_policy>`

Set what we should do with IMAP traffic.

**default**: intercept

.. versionadded:: 1.9.4

.. _conf_auditor_imap_interception:

imap_interception
-----------------

**optional**, **type**: :ref:`smtp interception <conf_value_dpi_imap_interception>`

Set the IMAP Interception config options.

**default**: set with default value

.. versionadded:: 1.9.7

icap_reqmod_service
-------------------

**optional**, **type**: :ref:`icap service config <conf_value_audit_icap_service_config>`

Set the ICAP REQMOD service config.

**default**: not set

.. versionadded:: 1.7.3

icap_respmod_service
--------------------

**optional**, **type**: :ref:`icap service config <conf_value_audit_icap_service_config>`

Set the ICAP RESPMOD service config.

**default**: not set

.. versionadded:: 1.7.3

.. _conf_auditor_stream_detour_service:

stream_detour_service
---------------------

**optional**, **type**: :ref:`stream detour service config <conf_value_audit_stream_detour_service_config>`

Set the :ref:`Stream Detour <protocol_helper_stream_detour>` service config.

You also need to change the inspect policy for each protocol to `detour` in order to really enable it.

If no stream detour service config set here, the protocols that is configured to use a `detour` policy will by bypassed.

**default**: not set

.. versionadded:: 1.9.8

.. _conf_auditor_task_audit_ratio:

task_audit_ratio
----------------

**optional**, **type**: :ref:`random ratio <conf_value_random_ratio>`

Set the task audit (like ICAP REQMOD/RESPMOD) ratio for incoming requests.

This also controls whether protocol inspection is really enabled for a specific request.

User side settings may override this.

**default**: 1.0, **alias**: application_audit_ratio

.. versionadded:: 1.7.4
