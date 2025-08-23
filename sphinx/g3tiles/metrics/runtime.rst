.. _metrics_runtime:

###############
Runtime Metrics
###############

The metrics for runtimes that support metrics.

The following are the tags for all logger metrics:

* :ref:`daemon_group <metrics_tag_daemon_group>`
* :ref:`stat_id <metrics_tag_stat_id>`

* runtime_id

  Show the runtime ID / label.

  There maybe many instances for the same runtime type, this field is used to distinguish between them.

.. _metrics_runtime_tokio:

Tokio Runtime Metrics
=====================

The metrics from tokio runtime.

* runtime.tokio.alive_tasks

  **type**: gauge

  Show the current number of alive tasks in the runtime.

* runtime.tokio.global_queue_depth

  **type**: gauge

  Show the number of tasks currently scheduled in the runtime's global queue.
