.. _metrics_backend_keyless:

#######################
Keyless Backend Metrics
#######################

Connection Metrics
==================

No extra tags.

The metric names are:

* backend.keyless.connection.attempt

  **type**: count

  Show the connect attempt count.

* backend.keyless.connection.established

  **type**: count

  Show the count of successful connection.

* backend.keyless.channel.alive

  **type**: gauge

  Show the alive channel numbers. The channel may be a TCP connection or a QUIC stream.

Request Metrics
===============

* backend.keyless.request.recv

  **type**: count

  Show the count of requests received.

* backend.keyless.request.send

  **type**: count

  Show the count of requests sent to the target peer.

* backend.keyless.request.drop

  **type**: count

  Show the count of requests that get dropped internally.

* backend.keyless.response.recv

  **type**: count

  Show the count of responses received from the target peer.

* backend.keyless.response.send

  **type**: count

  Show the count of responses sent to the client.

* backend.keyless.response.drop

  **type**: count

  Show the count of responses that get dropped internally.

Duration Metrics
================

The following tag is also set:

* :ref:`quantile <metrics_tag_quantile>`

The metric names are:

* backend.keyless.connect.duration

  **type**: gauge

  Show the connect duration stats.

* backend.keyless.wait.duration

  **type**: gauge

  Show the internal queue wait duration stats.

* backend.keyless.response.duration

  **type**: gauge

  Show the upstream response duration stats.
