.. _metrics_server:

##############
Server Metrics
##############

The metrics in server side shows the stats with client, and can be grouped to *request* and *traffic* types.

The following are the tags for all server metrics:

* :ref:`daemon_group <metrics_tag_daemon_group>`
* :ref:`stat_id <metrics_tag_stat_id>`

* server

  Show the server name.

* online

  Show if the server is online. The value is either 'y' or 'n'.

Listen
======

No extra tags.

The metric names are:

* listen.instance.count

  **type**: gauge

  Show how many listening sockets.

* listen.accepted

  **type**: count

  Show how many client connections has been accepted.

* listen.dropped

  **type**: count

  Show how many client connections has been dropped by acl rules at early stage.

* listen.timeout

  **type**: count

  Show how many client connections has been timed out in early protocol negotiation (such as TLS).

* listen.failed

  **type**: count

  Show how many times of accept error.

Task
====

A task is a keyless connection.

No other fixed tags. Extra tags set at server side will be added.

The metrics names are:

* server.task.total

  **type**: count

  Show how many valid tasks has been spawned. Each client connection will be promoted to a task.

* server.task.alive

  **type**: gauge

  Show how many alive tasks that spawned by this server are running. In normal case the daemon stopped by systemd,
  servers with running tasks will goto offline mode, and wait all tasks to be stopped.

Request
=======

Extra tags set at server side will be added.

The following are the extra tags for all request metrics:

* request

  Keyless request type. Available for all request metrics.

  The values are:

    - no_op
    - ping_pong
    - rsa_decrypt
    - rsa_sign
    - rsa_pss_sign
    - ecdsa_sign
    - ed25519_sign

* reason

  Keyless request failure reason.

  The values are:

    - key_not_found
    - crypto_fail
    - bad_op_code
    - format_error
    - other_fail

* :ref:`quantile <metrics_tag_quantile>`

The metric names are:

* server.request.total

  **type**: count

  Show the total count of new requests.

* server.request.alive

  **type**: gauge

  Show the keyless requests that is in processing.

* server.request.passed

  **type**: count

  Show the count of passed keyless requests.

* server.request.failed

  **type**: count

  Show the count of failed keyless requests. The tag **reason** will be added.

* server.request.duration

  **type**: gauge

  Show the histogram stats for keyless request process duration, which is corresponding to the
  :ref:`process_time <log_request_process_time>` field in logs.
