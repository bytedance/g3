
.. _configure_quic_value_types:

****
QUIC
****

.. _conf_value_quinn_transport:

Quinn Transport
===============

**yaml value**: map

The transport config to be used with quinn.

The map is consists of the following fields:

* max_idle_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Maximum duration of inactivity to accept before timing out the connection.
  The true idle timeout is the minimum of this and the peer's own max idle timeout.

  **default**: 60s

* keep_alive_interval

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Period of inactivity before sending a keep-alive packet.
  Must be set lower than the idle_timeout of both peers to be effective.

  **default**: 10s

* stream_receive_window

  **optional**, **type**: :ref:`humanize u32 <conf_value_humanize_u32>`

  Maximum number of bytes the peer may transmit without acknowledgement on any one stream before becoming blocked.
  This should be set to at least the expected connection latency multiplied by the maximum desired throughput.

  **default**: quinn default value

* receive_window

  **optional**, **type**: :ref:`humanize u32 <conf_value_humanize_u32>`

  Maximum number of bytes the peer may transmit across all streams of a connection before becoming blocked.
  This should be set to at least the expected connection latency multiplied by the maximum desired throughput.

  **default**: quinn default value

* send_window

  **optional**, **type**: :ref:`humanize u32 <conf_value_humanize_u32>`

  Maximum number of bytes to transmit to a peer without acknowledgment.

  **default**: quinn default value

.. versionadded:: 0.3.5
