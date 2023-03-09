.. _log_task:

********
Task Log
********

Each valid request will be a task. Each task will generate one log when finished.

Shared Keys
===========

The following shared keys are set in all type of task logs:

server_type
-----------

**required**, **type**: enum string

The type of the server that accepted the request.

server_name
-----------

**required**, **type**: string

The name of the server that accepted the request.

task_type
---------

**required**, **type**: enum string

The subtype of this task log. The meaning of non-shared keys are depend on this value.

task_id
-------

**required**, **type**: uuid in simple string format

UUID of the task.

The *task_id* will appear in other logs such as escape log if they have any association with this task.

stage
-----

**required**, **type**: enum string

The stage of the task.

The values available for each task depend on the server protocol. Here is all values:

* Created

  The task has just been created.

* Preparing

  We are preparing internal resources.

* Connecting

  We are trying to connect to remote peer.

* Connected

  We have just connected to remote peer.

* Replying

  We are trying to reply to clients that we have connected to remote peer.

* LoggedIn

  The upstream needs login and we have logged in.

* Relaying

  Both client and remote channel established, we are relaying data now.

* Finished

  The task has finished with no error. Only available for layer 7 protocols.

start_at
--------

**required**, **type**: rfc3339 timestamp string with microseconds

The time that the task is created (after validation).

.. note:: Not every request will be a task, only the valid ones.

user
----

**optional**, **type**: string

The username. Set only if user auth is enabled on server.

escaper
-------

**optional**, **type**: string

The selected escaper name.

reason
------

**required**, **type**: enum string

The brief reason why the task ends.

See the definition of **ServerTaskError** in code file *src/serve/error.rs*.

wait_time
---------

**optional**, **type**: time duration string

Show how many time spent from the acceptation of the request to the creation of the task.

For requests that reuse old connection, the start time will be the time we start to polling the next request,
so you may see very large wait_time in logs. This behaviour may change in future.

ready_time
----------

**optional**, **type**: time duration string

Show how many time spent from the creation of the task to the relaying stage, which means both the client channel
and the remote channel have been established. The value may be empty if the task failed early.

total_time
----------

**required**, **type**: time duration string

Show the time from the creation of the task to the end of the task.

Sub Types
=========

.. toctree::
   :maxdepth: 2

   tcp_connect
   http_forward
   ftp_over_http
   udp_associate
   udp_connect
