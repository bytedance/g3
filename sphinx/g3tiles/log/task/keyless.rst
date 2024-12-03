.. _log_task_keyless:

*******
Keyless
*******

The following keys are available for Keyless task log:

server_addr
-----------

**required**, **type**: socket address string

The listening address of the server.

client_addr
-----------

**required**, **type**: socket address string

The client address.

req_total
---------

**required**, **type**: usize

Total requests.

req_pass
--------

**required**, **type**: usize

Passed requests.

req_fail
--------

**required**, **type**: usize

Failed requests.

rsp_drop
--------

**required**, **type**: usize

Dropped responses.

rsp_pass
--------

**required**, **type**: usize

Passed responses.

rsp_fail
--------

**required**, **type**: usize

Failed responses.
