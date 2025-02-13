.. _configure_runtime_value_types:

*******
Runtime
*******

.. _conf_value_cpu_id_list_str:

cpu id list str
===============

A string the represent a list of CPU IDs.

It could be:

 - A single CPU ID
 - CPU ID range in the form `<start>-<end>`, where `start` should be less than `end`.
 - A list of CPU ID / CPU ID range delimited by ','

.. versionadded:: 1.11.3

.. _conf_value_cpu_set:

cpu set
=======

**yaml value** seq | str | usize

A `CPU_SET(3)`_ for use with `sched_setaffinity(2)`_.

The value should be one or a sequence of CPU IDs.

The CPU ID valid can be:

 - usize: a single CPU ID
 - string: :ref:`cpu id list str <conf_value_cpu_id_list_str>`

.. _CPU_SET(3): https://man7.org/linux/man-pages/man3/CPU_SET.3.html
.. _sched_setaffinity(2): https://man7.org/linux/man-pages/man2/sched_setaffinity.2.html

.. versionadded:: 1.3.1
.. versionchanged:: 1.11.3 allow a list of CPU ID string values

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

**default**: the number of logic CPU cores, **alias**: threads_total, thread_number_total

thread_number_per_runtime
-------------------------

**optional**, **type**: non-zero usize

Set the thread number that each tokio runtime should use.

**default**: 1, **alias**: threads_per_runtime

.. versionadded:: 1.11.3

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

For bool value:

* if true

  - if found any `WORKER_<N>_CPU_LIST` environment variables

    it will set the CPU affinity for that corresponding runtime `<N>`, the value should be :ref:`cpu id list str <conf_value_cpu_id_list_str>`.

    .. versionadded:: 1.11.3

  - otherwise if thread_number_per_runtime is set to 1

    a default CPU SET will be set for each thread, the CPU ID in the set will match the thread ID.

* if false, no sched affinity will be set, just as if this config option is not present.

**default**: no sched affinity set

.. versionadded:: 1.3.1

max_io_events_per_tick
----------------------

**optional**, **type**: usize

Configures the max number of events to be processed per tick.

**default**: 1024, tokio default value

.. versionadded:: 1.7.6
