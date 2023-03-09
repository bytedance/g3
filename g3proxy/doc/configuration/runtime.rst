.. _configuration_runtime:

*******
Runtime
*******

This is the *runtime* config, which is optional. If set, it must reside in the main conf file.

All the options in this config are optional with a reasonable default value.
Set them only if you really known their meaning.

The options can be grouped into the following sections:

tokio main runtime
==================

This section describes the options for the main tokio runtime, which is used for all servers.

thread_number
-------------

**optional**, **type**: int | str

Set the scheduler and core number of worker threads.

if *0*, a basic scheduler is used.
if not *0*, a threaded scheduler with the specified number of worker thread is used.

**default**: threaded scheduler with worker threads on each all available CPU core.

thread_name
-----------

**optional**, **type**: str

Set name of worker threads spawned. Only ASCII characters is allowed.
Note that the length of thread name will be restricted at the OS level.

**default**: "tokio"

thread_stack_size
-----------------

**optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

Set the stack size for worker threads. For *<int>* value, the unit is bytes.

**default**: `tokio thread_stack_size`_

.. _tokio thread_stack_size: https://docs.rs/tokio/0.2.21/tokio/runtime/struct.Builder.html#method.thread_stack_size

max_io_events_per_tick
----------------------

**optional**, **type**: usize

Configures the max number of events to be processed per tick.

**default**: 1024, tokio default value

.. versionadded: 1.7.6

daemon quit control
===================

This section describes the options used during graceful quit of the daemon.

server_offline_delay
--------------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the time duration before offline all servers after received daemon quit signals.
All listen server sockets will be closed after this duration, so it should be more than the time used to
start the new daemon process if you depends on it for graceful restart.

**default**: 4s

task_wait_delay
---------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the time duration before checking alive tasks after all servers going into offline mode.
Tasks are marked as alive only if auth success, so we should leave some time for those tasks in negotiation
state to run into their next state, which may be alive or really dead.

**default**: 2s

task_wait_timeout
-----------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the time duration before force quit alive tasks after we decide to wait for them to end gracefully.

**default**: 10h

task_quit_timeout
-----------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the time duration before we shutdown the process after entering force quit status for all tasks.
The tasks dropped after this timeout won't have any logs.
