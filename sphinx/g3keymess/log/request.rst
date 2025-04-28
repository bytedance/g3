.. _log_request:

***********
Request Log
***********

When a request failed, a request log will be generated.

The following keys will be set in request log.

server_name
-----------

**required**, **type**: string

The name of the server that accepted the request.

task_id
-------

**required**, **type**: uuid in simple string format

UUID of the task.

msg_id
------

**required**, **type**: usize string

The msg id field in the request.
