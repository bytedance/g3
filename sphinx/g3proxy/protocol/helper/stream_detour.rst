.. _protocol_helper_stream_detour:

=============
Stream Detour
=============

An external interception server can implement this to intercept protocols that are configured
with `detour` inspect policy in :ref:`auditor <configuration_auditor>` config, each protocol will have
a separate config option.

The external server should listen to a QUIC port, and configure it by setting
:ref:`stream detour service <conf_auditor_stream_detour_service>` in auditor config.

g3proxy will connect to this port to setup a lot if IDLE connections at the beginning,
And will open two bidirectional QUIC streams for a single client-remote stream when needed,
one is called north stream, another one called south stream.

North Stream
------------

The north stream will be used to forward data sending by client to remote.

Initially g3proxy will send a **ProxyProtocolV2 Header**, and an optional **Payload** to the server,

The PPv2 Type-Values are:

* 0xE0 | Upstream Address

  The target upstream address, encoded in UTF-8 without trailing '\0'.
  This will always be set.

* 0xE2 | Username

  The username of the client, encoded in UTF-8 without trailing '\0'.
  This will be set only if client auth is enabled.

* 0xE3 | Task ID

  The task id in UUID binary format. This will always be set.

* 0xE4 | Protocol

  The detected protocol string, encoded in UTF-8 without trailing '\0'.
  This will always be set.

  You will find the detail value in :ref:`Protocol and Payload <stream_detour_protocol_payload>` section.

* 0xE5 | Match ID

  The ID used to combine the north stream and the south stream.
  The value will be a 2-bytes uint16 value, in big-endian.

* 0xE6 | Payload Length

  Extra payload data length. The payload data format will be vary depending on the *protocol*.
  The value will be a 4-bytes uint32 value, in big-endian.
  The payload data will be sent right following the PPv2 Header if the length is greater than 0.

  You will find the payload format in :ref:`Protocol and Payload <stream_detour_protocol_payload>` section.

After sending the PPv2 header, g3proxy will waiting a 4-bytes response from the server.

- The first 2 bytes should be a uint16 value in big-endian.
- The last 2 bytes should be a uint16 action code in big-endian. The supported actions:

  * 0 - continue

    Continue to send data, the data flow will be `client_read -> detour_server -> remote_write`.

  * 1 - bypass

    Skip the detour server, transfer client - remote data directly.

  * 2 - block

    Block the client-remote transfer, close the connection immediately.

South Stream
------------

The south stream will be used to forward data sending by remote to client.

Initially g3proxy will send a ProxyProtocolV2 header to the server,

The PPv2 Type-Values are:

* 0xE5 | Match ID

  The ID used to combine the north stream and the south stream.
  The value will be a 2-bytes uint16 value, in big-endian.

After sending that PPv2 header, the data flow will be `remote_read -> detour_server -> client_write`.

.. _stream_detour_protocol_payload:

Protocol and Payload
--------------------

HTTP 2
^^^^^^

**protocol value**: http_2

**payload format**: no payload

WebSocket
^^^^^^^^^

**protocol value**: websocket

**payload format**:

The payload will be multiline of text, each line will be ended with "\r\n".

The first line will be the */resource name/*.

The following lines will be the same as the HTTP header lines used in HTTP Upgrade stage, the possible headers:

- Host in request
- Origin in request
- Sec-WebSocket-Key in request
- Sec-WebSocket-Version in request
- Sec-WebSocket-Accept in response
- Sec-WebSocket-Protocol in response
- Sec-WebSocket-Extensions in response

SMTP
^^^^

**protocol value**: smtp

**payload format**: no payload

IMAP
^^^^

**protocol value**: imap

**payload format**: no payload
