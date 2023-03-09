.. _configuration_log:

***
Log
***

This is the initial *log* config, which is optional and can not be reloaded.
If set, it must reside in the main conf file.

Root Value
==========

The value could be a simple string, which is the driver name, such as

- discard

  drop the logs. This is the **default**.

- journal

  send logs to journald directly.

- syslog

  send logs to syslogd directly.

In such case, a default driver is used as default log config for all loggers.

The value could be a map, with the following keys:

- default

  **optional**, **type**: :ref:`log config <configuration_log_config>`

  Set default log config for loggers with no explicit config.

  **default**: journal log config

- task

  **optional**, **type**: :ref:`log config <configuration_log_config>`

  Set log config for *task* loggers.

  **default**: not set

- escape

  **optional**, **type**: :ref:`log config <configuration_log_config>`

  Set log config for *escape* loggers.

  **default**: not set

- resolve

  **optional**, **type**: :ref:`log config <configuration_log_config>`

  Set log config for *resolve* loggers.

  **default**: not set

.. _configuration_log_config:

log config
==========

The detailed log config may be a simple driver name, or a map, with the following keys:

- journal

  **optional**, **type**: map

  Use *journal* log driver. An empty map should be used, as no keys are defined by now.

- syslog

  **optional**, **type**: :ref:`syslog <configuration_log_driver_syslog>`

  Use *syslog* log driver.

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

.. toctree::
   :maxdepth: 2
   :caption: More:

   driver/index
