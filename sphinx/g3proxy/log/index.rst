.. _log:

###
Log
###

We support many logging drivers, see :ref:`log <configuration_log>` for more details.

All generated logs is structured, we will describe the structures of all type of logs we generated in this doc.

Shared Keys
===========

The following shared keys are set in all type of logs:

daemon_name
-----------

**optional**, **type**: string

The daemon group name of the process, which can be set by using of command line options.

pid
---

**required**, **type**: int

The pid of the process.

There may be many processes running, one online and the others in offline mode.

log_type
--------

**required**, **type**: enum string

Show the log type. The meaning of non-shared keys are depend on this value.

Values are:

  * Task
  * Escape
  * Resolve

.. _log_shared_keys_report_ts:

report_ts
---------

**optional**, **type**: unix timestamp

Show the timestamp when we generate this log.

It will be present if the log driver has been configured to append it, see :ref:`log driver <configuration_log_driver>`
for more info.

Log Types
=========

.. toctree::

   task/index
   escape/index
   resolve/index
