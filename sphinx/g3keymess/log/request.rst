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

create_at
---------

**required**, **type**: rfc3339 timestamp string with microseconds

The create datetime of this request.

.. versionadded:: 0.4.2

.. _log_request_process_time:

process_time
------------

**required**, **type**: time duration string

The time spend to process this request.

.. versionadded:: 0.4.2
