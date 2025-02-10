.. _configure_runtime_value_types:

*******
Runtime
*******

.. _conf_value_cpu_set:

cpu set
=======

**yaml value** seq | usize

A `CPU_SET(3)`_ for use with `sched_setaffinity(2)`_.

The value should be a CPU ID, starting from 0, or a sequence of CPU IDs.

.. _CPU_SET(3): https://man7.org/linux/man-pages/man3/CPU_SET.3.html
.. _sched_setaffinity(2): https://man7.org/linux/man-pages/man2/sched_setaffinity.2.html

.. _conf_value_unaided_runtime_config:

unaided runtime config
======================

**yaml value**: map

This is the config for unaided runtime.

The keys are:

thread_number
-------------

**optional**, **type**: non-zero usize

Set the total thread number.

**default**: the number of logic CPU cores **alias**: threads_total, thread_number_total

thread_number_per_runtime
-------------------------

**optional**, **type**: non-zero usize

Set the thread number that each tokio runtime should use.

**default**: 1, **alias**: threads_per_runtime

.. versionadded:: 0.3.8

thread_stack_size
-----------------

**optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

Set the stack size for worker threads. For *<int>* value, the unit is bytes.

**default**: system default

sched_affinity
--------------

**optional**, **type**: map | bool

Set the sched affinity for each threads.

For map value, the key should be the thread id starting from 0, and the value should be :ref:`cpu set <conf_value_cpu_set>`.

For bool value (only if thread_number_per_runtime is set to 1):

* if true, a default CPU SET will be set for each thread, the CPU ID in the set will match the thread ID.

* if false, no sched affinity will be set, just as if this config option is not present.

**default**: no sched affinity set

max_io_events_per_tick
----------------------

**optional**, **type**: usize

Configures the max number of events to be processed per tick.

**default**: 1024, tokio default value

openssl_async_job_init_size
---------------------------

**optional**, **type**: usize

Set initial openssl asynchronous job size for the current thread. See `ASYNC_start_job`_ for more details.

.. note:: No effect if thread_number_per_runtime is set to greater than 1.

**default**: 0

.. versionadded:: 0.3.8

openssl_async_job_max_size
--------------------------

**optional**, **type**: usize

Set max openssl asynchronous job size for the current thread. See `ASYNC_start_job`_ for more details.

.. note:: No effect if thread_number_per_runtime is set to greater than 1.

**default**: 0

.. versionadded:: 0.3.8

.. _ASYNC_start_job: https://docs.openssl.org/master/man3/ASYNC_start_job/
