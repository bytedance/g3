.. _configuration_log:

***
Log
***

This is the config for event logs, which is optional and can not be reloaded.
If set, the *Root Value* below must reside in the main conf file.

Root Value
==========

The value could be a simple string, which is the driver name, such as

- discard

  drop the logs. This is the **default**.

- journal

  send logs to journald directly.

- syslog

  send logs to syslogd directly.

- stdout

  send logs to stdout.

In such case, a default driver is used as default log config for all loggers.

The value could be a map, with the following keys:

- default

  **optional**, **type**: :ref:`log config <configuration_log_config>`

  Set default log config for loggers with no explicit config.

  **default**: discard

- syslog

  **optional**, **type**: :ref:`syslog <configuration_log_driver_syslog>`

  Set default log config for loggers with no explicit config.

  **default**: not set

- fluentd

  **optional**, **type**: :ref:`fluentd <configuration_log_driver_fluentd>`

  Set default log config for loggers with no explicit config.

  **default**: not set

- task

  **optional**, **type**: :ref:`log config <configuration_log_config>`

  Set log config for *task* loggers.

  **default**: not set

.. _configuration_log_config:

Log Config Value
================

The detailed log config may be a simple driver name, or a map, with the following keys:

- journal

  **optional**, **type**: map

  Use *journal* log driver. An empty map should be used, as no keys are defined by now.

- syslog

  **optional**, **type**: :ref:`syslog <configuration_log_driver_syslog>`

  Use *syslog* log driver.

- fluentd

  **optional**, **type**: :ref:`fluentd <configuration_log_driver_fluentd>`

  Use *fluentd* log driver.

- async_channel_size

  **optional**, **type**: usize

  Set the internal async channel size.

  **default**: 4096

- async_thread_number

  **optional**, **type**: usize

  Set the number of async threads.

  This has no effect on *discard* and *journal* log driver.

  **default**: 1

- io_error_sampling_offset

  **optional**, **type**: usize, **max**: 16

  The logger may encounter io error, we should report it anyhow. We will log this error every *2^n* times,
  where *n* can be set here.

  This has no effect on *discard* and *journal* log driver.

  **default**: 10

.. note:: The *discard* driver has no config options, so it doesn't has a corresponding map field.

.. _configuration_log_driver:

Drivers
=======

- discard
- stdout
- systemd journal
- :doc:`driver/syslog`
- :doc:`driver/fluentd`

.. toctree::
   :hidden:
   :glob:

   driver/*
