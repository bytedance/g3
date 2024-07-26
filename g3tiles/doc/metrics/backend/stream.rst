.. _metrics_backend_stream:

######################
Stream Backend Metrics
######################

Connection Metrics
==================

No extra tags.

The metric names are:

* backend.stream.connection.attempt

  **type**: count

  Show the connect attempt count.

* backend.stream.connection.established

  **type**: count

  Show the count successful connection.

Duration Metrics
================

The following tag is also set:

* :ref:`quantile <metrics_tag_quantile>`

The metric names are:

* backend.stream.connect.duration

  **type**: gauge

  Show the tcp connect ready time duration.
