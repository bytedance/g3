.. _configuration_backend:

*******
backend
*******

You can set keyless backend config in this section.

The backend config can be a root value map as described below, or just the driver name.

Root Value Map
==============

dispatch_channel_size
---------------------

**optional**, **type**: usize

Set the channel size for the dispatch of requests to worker backend.

This will only have effect when worker is enabled in main conf.

**default**: 1024

dispatch_counter_shift
----------------------

**optional**, **type**: u8

Set the count of the requests that will be dispatched to the same worker backend before rotate to the next one.

The count value will be 2^N.

This will only have effect when worker is enabled in main conf.

**default**: 3

openssl_async_job
-----------------

**optional**, **type**: :ref:`openssl_async_job <conf_backend_driver_openssl_async_job>`

Use OpenSSL Async Job driver.

**default**: not enabled

Drivers
=======

simple
------

Use OpenSSL default mode for Private Key operations.

There is no extra config for this driver.

.. _conf_backend_driver_openssl_async_job:

openssl_async_job
-----------------

Use OpenSSL async job for Private Key operations. You can set the hardware crypto engine to use in openssl.cnf.

The following keys are supported for this driver:

- async_op_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout for a single async job.

  It is recommended to set a large value to avoid use-after-free crash in OpenSSL Async Job code.

  **default**: 1s
