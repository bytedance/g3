
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

TLS Interception
================

.. _conf_value_dpi_tls_cert_generator:

tls cert generator
------------------

**type**: map

Set the config for tls certificate generator.

The keys are:

* query_peer_addr

  **optional**, **type**: :ref:`sockaddr str <conf_value_sockaddr_str>`

  Set the peer udp socket address.

  **default**: 127.0.0.1:2999

* query_socket_buffer

  **optional**, **type**: :ref:`socket buffer config <conf_value_socket_buffer_config>`

  Set the socket buffer config for the socket to peer.

  **default**: not set

* query_wait_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout for the cache runtime to wait response from the query runtime.

  **default**: 400ms

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

  **default**: 800ms

* cache_vanish_wait

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the vanish time after the record is considered expired (not the certificate expire time).

  **default**: 300s

.. _conf_value_dpi_tls_interception_client:

tls interception client
-----------------------

**type**: map

Set the tls client config for tls interception.

The keys are:

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
