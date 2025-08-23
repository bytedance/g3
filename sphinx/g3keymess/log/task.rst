.. _log_task:

********
Task Log
********

Each client connection will be handled as a task, and will emit a task log when the connection finished.

The following keys will be set in task log.

server_name
-----------

**required**, **type**: string

The name of the server that accepted the request.

task_id
-------

**required**, **type**: uuid in simple string format

UUID of the task.

The *task_id* will appear in other logs such as request log if they have any association with this task.

server_addr
-----------

**required**, **type**: socket address string

The listening address of the server.

client_addr
-----------

**required**, **type**: socket address string

The client address.

start_at
--------

**required**, **type**: rfc3339 timestamp string with microseconds

The time that the task is created (after validation).

.. note:: Not every request will be a task, only the valid ones.
