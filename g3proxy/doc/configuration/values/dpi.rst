.. _configure_dpi_value_types:

***
DPI
***

All dpi value types are described here.

Protocol Inspection
===================

.. _conf_value_dpi_inspection_size_limit:

inspection size limit
---------------------

**type**: map

This will set size limit for each protocol with no explicit size limit in their specification.

The keys ars:

* ftp_greeting_msg

  **optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

  Set for FTP server greeting message.

  **default**: 512

* http_request_uri

  **optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

  Set for HTTP client request URI.

  **default**: 4096

* imap_greeting_msg

  **optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

  Set for IMAP server greeting message.

  **default**: 512

* nats_info_line

  **optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

  Set for NATS server info line.

  **default**: 1024

* smtp_greeting_msg

  **optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

  Set for SMTP server greeting message.

  **default**: 512

  .. deprecated:: 1.9.0 not used anymore, the max SMTP reply line length should be 512

.. _conf_value_dpi_protocol_inspect_action:

protocol inspect action
-----------------------

**type**: string

Set what we should do to a specific application protocol.

The possible values for this are:

- block

  Block the traffic. And we will try to send application level error code to the client.

- intercept

  Intercept the traffic. This is the default value.

- bypass

  Bypass the interception. The traffic will be transferred transparently.

- detour

  Send the traffic to a stream detour service, which will be configured at somewhere in the context.

.. versionadded:: 1.9.9

.. _conf_value_inspect_rule:

inspect rule
------------

**yaml value**: map

All the rules share the same config format described in this section.

An inspect rule is consisted of many records, each of them has an associated
:ref:`protocol inspect action <conf_value_dpi_protocol_inspect_action>`.

The value in map format is consisted of the following fields:

* any of the protocol inspect actions as the key str

  The value should be a valid record or a list of them, with the key string as the acl action.
  See detail types for the format of each record type.

.. versionadded:: 1.9.9

.. _conf_value_dst_subnet_inspect_rule:

dst subnet inspect rule
-----------------------

**yaml value**: :ref:`inspect rule <conf_value_inspect_rule>`

The record type should be :ref:`ip network str <conf_value_ip_network_str>`.

.. versionadded:: 1.9.9

.. _conf_value_exact_host_inspect_rule:

exact host inspect rule
-----------------------

**yaml value**: :ref:`inspect rule <conf_value_inspect_rule>`

The record type should be :ref:`host <conf_value_host>`.

.. versionadded:: 1.9.9

.. _conf_value_child_domain_inspect_rule:

child domain inspect rule
-------------------------

**yaml value**: :ref:`inspect rule <conf_value_inspect_rule>`

Specify the parent domain to match, all children domain in this domain will be matched.

The record type should be :ref:`domain <conf_value_domain>`.

.. versionadded:: 1.9.9

.. _conf_value_dpi_protocol_inspect_policy:

protocol inspect policy
-----------------------

**yaml value**: string | map

This rule set is used to match dst host for each protocol inspection call.

Consisted of the following rules:

* default

  **optional**,  **type**: :ref:`protocol inspect action <conf_value_dpi_protocol_inspect_action>`

  Set the default inspect action if no rules matched explicitly.

* exact_match

  **optional**,  **type**: :ref:`exact host inspect rule <conf_value_exact_host_inspect_rule>`

* child_match

  **optional**,  **type**: :ref:`child domain inspect rule <conf_value_child_domain_inspect_rule>`

  Match only if the host is a domain.

* subnet_match

  **optional**,  **type**: :ref:`dst subnet inspect rule <conf_value_dst_subnet_inspect_rule>`

  Match only if the host is an IP Address.

The match order is the same as the list order above.

One can use the *string* type to define a default action for any upstream traffic, regardless of the host,

.. versionadded:: 1.9.9

.. _conf_value_dpi_protocol_inspection:

protocol inspection
-------------------

**type**: map

This set the basic protocol inspection config.

The keys are:

* inspect_max_depth

  **optional**, **type**: usize

  Set the max inspection depth. The stream will be treated as unknown protocol if it's nested too much.

  **default**: 4

* data0_buffer_size

  **optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

  Set the stream buffer size for protocol inspection.

  **default**: 4096

* data0_wait_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the wait timeout for the initial data, from either the client side or the server side.

  **default**: 60s

.. _conf_value_dpi_protocol_inspection_data0_read_timeout:

* data0_read_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the read timeout value when reading initial data for protocol inspection after it's arrival.

  If timeout, the protocol will be considered as unknown.

  **default**: 4s

* data0_size_limit

  **optional**, **type**: :ref:`inspection size limit <conf_value_dpi_inspection_size_limit>`

  Set inspection size limit for each protocol.

  **default**: set with default value

.. _conf_value_dpi_maybe_protocol:

maybe protocol
--------------

**type**: str

The following values are supported:

* http
* https
* smtp
* ssh
* ftp
* pop3
* pop3s
* nntp
* nntps
* imap
* imaps
* nats
* bittorrent

.. _conf_value_dpi_portmap:

portmap
-------

**type**: seq | map

Set the protocol indication for each port.

For *seq* value, each element should be a map, with two keys:

* port

  **required**, **type**: u16

  Set the port number.

* protocol

  **required**, **type**: :ref:`maybe protocol <conf_value_dpi_maybe_protocol>` | seq

  Set the protocol(s).

For *map* value, the key should be the port, and the value should be the same as the *protocol* above.

.. _conf_value_dpi_server_tcp_portmap:

server tcp portmap
------------------

**type**: :ref:`portmap <conf_value_dpi_portmap>`

Set the protocol indication for each server side tcp port.

See the code `lib/g3-dpi/src/protocol/portmap.rs` for default set ports.

.. _conf_value_dpi_client_tcp_portmap:

client tcp portmap
------------------

**type**: :ref:`portmap <conf_value_dpi_portmap>`

Set the protocol indication for each client side tcp port.

See the code `lib/g3-dpi/src/protocol/portmap.rs` for default set ports.

.. _conf_value_dpi_stream_dump:

stream dump
-----------

**type**: map | str

Set stream dump config. You can use this to dump streams to :ref:`wireshark udpdump <protocol_setup_wireshark_udpdump>`.

The keys are:

* peer

  **required**, **type**: :ref:`env sockaddr str <conf_value_env_sockaddr_str>`

  Set the peer udp socket address.

  **default**: 127.0.0.1:5555

* socket_buffer

  **optional**, **type**: :ref:`socket buffer config <conf_value_socket_buffer_config>`

  Set the buffer config for the udp socket.

  **default**: not set

* misc_opts

  **optional**, **type**: :ref:`udp misc sock opts <conf_value_udp_misc_sock_opts>`

  Set misc udp socket options on created udp sockets.

  **default**: not set

* packet_size

  **optional**, **type**: usize

  Set the max udp packet size.

  **default**: 1480

* client_side

  **optional**, **type**: bool

  Set this to true to dump client side traffic.

  **default**: false, the remote side traffic will be dumped

  .. versionadded:: 1.9.7

TLS Interception
================

.. _conf_value_dpi_tls_cert_agent:

tls cert agent
--------------

**type**: map | str

Set the config for tls certificate agent / generator.

The keys are:

* query_peer_addr

  **optional**, **type**: :ref:`env sockaddr str <conf_value_env_sockaddr_str>`

  Set the peer udp socket address.

  **default**: 127.0.0.1:2999

* query_socket_buffer

  **optional**, **type**: :ref:`socket buffer config <conf_value_socket_buffer_config>`

  Set the socket buffer config for the socket to peer.

  **default**: not set

* query_wait_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout for the cache runtime to wait response from the query runtime.

  **default**: 4s

.. _conf_value_dpi_tls_cert_agent_protective_cache_ttl:

* protective_cache_ttl

  **optional**, **type**: u32

  Set the protective cache ttl for certificates returned by peer.

  **default**: 10

* maximum_cache_ttl

  **optional**, **type**: u32

  Set the maximum cache ttl for certificates returned by peer.

  **default**: 300

* cache_request_batch_count

  **optional**, **type**: usize

  Set the batch request count in cache runtime.

  **default**: 10

* cache_request_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the request timeout for the caller.

  **default**: 4s

.. _conf_value_dpi_tls_cert_agent_cache_vanish_wait:

* cache_vanish_wait

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the vanish time after the record is considered expired (not the certificate expire time).

  **default**: 300s

For *str* value, it will parsed as *query_peer_addr* and use default value for other fields.

.. versionchanged:: 1.7.11 allow str value

.. _conf_value_dpi_tls_interception_client:

tls interception client
-----------------------

**type**: map

Set the tls client config for tls interception.

The keys are:

* min_tls_version

  **optional**, **type**: :ref:`tls version <conf_value_tls_version>`

  Set the minimal TLS version to use.

  **default**: not set

  .. versionadded:: 1.9.9

* max_tls_version

  **optional**, **type**: :ref:`tls version <conf_value_tls_version>`

  Set the maximum TLS version to use.

  **default**: not set

  .. versionadded:: 1.9.9

* ca_certificate

  **optional**, **type**: :ref:`tls certificates <conf_value_tls_certificates>`

  Add CA certificate for certificate verification of the upstream server.

  **default**: not set

* no_default_ca_certificate

  **optional**, **type**: false

  Set if we should not load the system default CA certificates.

  **default**: false

* handshake_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout for upstream tls handshake.

  **default**: 10s

* no_session_cache

  **optional**, **type**: bool

  Set if we should disable tls session cache.

  **default**: false

* session_cache_lru_max_sites

  **optional**, **type**: usize

  Set how many LRU sites should have cached sessions.

  **default**: 128

* session_cache_each_capacity

  **optional**, **type**: usize

  Set how many sessions should be kept for each site.

  **default**: 16

* supported_groups

  **optional**, **type**: str

  Set the supported elliptic curve groups.

  **default**: not set

  .. versionadded:: 1.7.35

* use_ocsp_stapling

  **optional**, **type**: bool

  Set this to true to request a stapled OCSP response from the server.

  Verify of this response is still not implemented.

  **default**: false

  .. versionadded:: 1.7.35

* enable_sct

  **optional**, **type**: bool

  Enable the processing of signed certificate timestamps (SCTs) for OpenSSL, or enables SCT requests for BoringSSL.

  Verify of this response is still not implemented for BoringSSL variants.

  **default**: not set, the default value may vary between different OpenSSL variants

  .. versionadded:: 1.7.35

* enable_grease

  **optional**, **type**: bool

  Enable GREASE. See `RFC 8701`_.

  **default**: not set, the default value may vary between different OpenSSL variants

  .. versionadded:: 1.7.35

  .. _RFC 8701: https://datatracker.ietf.org/doc/rfc8701/

* permute_extensions

  **optional**, **type**: bool

  Whether to permute TLS extensions.

  **default**: not set, the default value may vary between different OpenSSL variants

  .. versionadded:: 1.7.36

.. _conf_value_dpi_tls_interception_server:

tls interception server
-----------------------

.. versionadded:: 1.7.36

**type**: map

Set the tls server config for tls interception.

The keys are:

* accept_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout for client tls handshake.

  This timeout value is also used for accepting the initial ClientHello message.

  **default**: 10s, **alias**: handshake_timeout

HTTP Interception
=================

.. _conf_value_dpi_h1_interception:

h1 interception
---------------

**type**: map

Set the config for HTTP 1.x interception.

The keys are:

* pipeline_size

  **optional**, **type**: usize

  Set the pipeline size.

  **default**: 10

* pipeline_read_idle_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the idle timeout of the client side IDLE http connections.

  **default**: 5min

* req_header_recv_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the max time to wait a full request header after the client connection become readable.

  **default**: 30s

* rsp_header_recv_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the max time duration after the full request sent and before receive of the whole response header.

  **default**: 60s

* req_header_max_size

  **optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

  Set the max request header size.

  **default**: 64KiB

* rsp_header_max_size

  **optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

  Set the max response header size.

  **default**: 64KiB

* body_line_max_length

  **optional**, **type**: int

  Set the max line length for lines (trailer and chunk size) in http body.

  **default**: 8192

* steal_forwarded_for

  **optional**, **type**: bool

  Set if we should delete the *Forwarded* and *X-Forwarded-For* headers from the client's intercepted transparent request.

  **default**: false

  .. versionadded:: 1.9.2

.. _conf_value_dpi_h2_interception:

h2 interception
---------------

**type**: map

Set the config for HTTP 2.0 interception.

The keys are:

* max_header_list_size

  **optional**, **type**: :ref:`humanize u32 <conf_value_humanize_u32>`

  Set the max header size.

  **default**: 64KiB

* max_concurrent_streams

  **optional**, **type**: u32

  Set the max concurrent stream for each http2 connection.

  **default**: 16

* max_frame_size

  **optional**, **type**: :ref:`humanize u32 <conf_value_humanize_u32>`

  Set the max frame size.

  **default**: 1MiB

* max_send_buffer_size

  **optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

  Set the max send buffer size.

  **default**: 16MiB

* disable_upstream_push

  **optional**, **type**: bool

  Set if we should disable server push.

  **default**: false

* upstream_handshake_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the http2 handshake timeout to upstream.

  **default**: 10s

* upstream_stream_open_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the upstream stream open timeout.

  **default**: 10s

* client_handshake_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the http2 handshake timeout to client.

  **default**: 4s

* rsp_header_recv_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the max time duration after the full request sent and before receive of the whole response header.

  **default**: 60s

* silent_drop_expect_header

  **optional**, **type**: bool

  Set if we should drop the *Expect* http header silently.
  If not set, a *417 Expectation Failed* response will be sent to client.

.. _conf_value_dpi_smtp_interception:

smtp interception
-----------------

* greeting_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout value for the forward of the upstream SMTP Greeting message.

  **default**: 5min

* quit_wait_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout value for the forward of the upstream QUIT response.

  **default**: 60s

* command_wait_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout value for the wait of the next client SMTP command.

  **default**: 5min

* response_wait_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout value for the wait of the most of upstream SMTP command response.

  **default**: 5min

* data_initiation_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout value for the initial confirm response to DATA command from upstream.

  **default**: 2min

* data_termination_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout value for the final status response to DATA command from upstream.

  **default**: 10min

* allow_on_demand_mail_relay

  **optional**, **type**: bool

  Set whether we should enable `rfc2645 ODMR`_ protocol support.

  .. note:: Interception for the SMTP connection inside ODMR is currently not supported.

  **default**: false

* allow_data_chunking

  **optional**, **type**: bool

  Set whether we should enable `rfc3030 BDAT`_ command support.

  .. note:: ICAP integration is not available currently.

  **default**: false

* allow_burl_data

  **optional**, **type**: bool

  Set whether we should enable `rfc4468 BURL`_ command support.

  .. note:: ICAP integration is not available currently.

  **default**: false

.. _rfc2645 ODMR: https://datatracker.ietf.org/doc/html/rfc2645
.. _rfc3030 BDAT: https://datatracker.ietf.org/doc/html/rfc3030
.. _rfc4468 BURL: https://datatracker.ietf.org/doc/html/rfc4468

.. versionadded:: 1.9.2

.. _conf_value_dpi_imap_interception:

imap interception
-----------------

* greeting_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout value for the forward of the upstream IMAP Greeting message.

  **default**: 5min

* authenticate_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the total time to wait before the connection enter authenticated state.

  **default**: 5min

* logout_wait_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout value for the forward of the upstream LOGOUT response.

  **default**: 10s

* command_line_max_size

  **optional**, **type**: usize

  Set the max size for a single IMAP command line.

  **default**: 4096

* response_line_max_size

  **optional**, **type**: usize

  Set the max size for a single IMAP response line.

  **default**: 4096

* forward_max_idle_count

  **optional**, **type**: i32

  Set the max IDLE count allowed when forwarding IMAP command/response lines, including IMAP IDLE state.

  The IDLE check interval will be :ref:`task_idle_check_duration <conf_server_common_task_idle_check_duration>`.

  **default**: 6

* transfer_max_idle_count

  **optional**, **type**: i32

  Set the max IDLE count allowed when transferring IMAP command/response literal data.

  The IDLE check interval will be :ref:`task_idle_check_duration <conf_server_common_task_idle_check_duration>`.

  **default**: 1

.. versionadded:: 1.9.7
