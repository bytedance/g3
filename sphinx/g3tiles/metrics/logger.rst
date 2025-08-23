.. _metrics_logger:

##############
Logger Metrics
##############

The metrics for loggers that support metrics.

The *discard* and *journal* log driver do not support metrics, see config for :ref:`log <configuration_log>`.

The following are the tags for all logger metrics:

* :ref:`daemon_group <metrics_tag_daemon_group>`
* :ref:`stat_id <metrics_tag_stat_id>`

* logger

  Show the name of the logger.

The metrics are:

* logger.message.total

  **type**: count

  Show the total number of logs.

* logger.message.pass

  **type**: count

  Show the number of logs passed to next peer.

* logger.traffic.pass

  **type**: count

  Show the bytes of log that has been sent to next peer.

* logger.message.drop

  Show the number of logs that has been dropped.

  An extra tag **drop_type** is used to add more details for the drop reason, values are:

  - FormatFailed: the message can not be formatted to match the real log protocol

  - ChannelClosed: the internal async channel has been closed.

  - ChannelOverflow: the internal async channel is full.

  - PeerUnreachable: the next peer is closed or currently unreachable.
