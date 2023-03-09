
.. _configure_metrics_value_types:

*******
Metrics
*******

.. _conf_value_metrics_value:

metrics value
=============

**yaml value**: limited str

Only the following characters are allowed:

a to z, A to Z, 0 to 9, -, _, ., / or Unicode letters (as per the specification)

The character range is the same as `OpenTSDB metrics-and-tags`_.

.. _OpenTSDB metrics-and-tags: http://opentsdb.net/docs/build/html/user_guide/writing/index.html#metrics-and-tags

.. _conf_value_static_metrics_tags:

static metrics tags
===================

**yaml value**: map

This should be a map, each key will be the tag name, and it's value will be the tag value.

The tag name and the tag value should be of type :ref:`metrics value <conf_value_metrics_value>`.

.. _conf_value_metrics_name:

metrics name
============

**yaml value**: :ref:`metrics value <conf_value_metrics_value>`

The metrics name


.. _conf_value_statsd_client_config:

Statsd Client Config
====================

The full format of the root value should be a map, with the following keys:

target_unix
-----------

**optional**, **type**: mix

You can set this if you want to send statsd metrics to a custom unix socket path.

The value can be a map, with the following keys:

* path

  **required**, **type**: :ref:`absolute path <conf_value_absolute_path>`

  The syslogd daemon listen socket path.

If the value type is str, the value should be the same as the value as *path* above.

**default**: not set

.. versionadded:: 1.3.5

target_udp
----------

**optional**, **type**: mix

You can set this if you want to send statsd metrics to a remote statsd which listening on a udp socket.

The value can be a map, with the following keys:

* address

  **optional**, **type**: :ref:`sockaddr str <conf_value_sockaddr_str>`

  Set the remote socket address.

  **default**: 127.0.0.1:8125

* bind_ip

  **optional**, **type**: :ref:`ip addr str <conf_value_ip_addr_str>`

  Set the ip address to bind to for the local socket.

  **default**: not set

If the value type is str, the value should be the same as the value as *address* above.

.. versionadded:: 1.3.5

target
------

**optional**, **type**: map

This is just another form to set statsd target address.

The key *udp* is just handled as *target_udp* as above.

The key *unix* is just handled as *target_unix* as above.

prefix
------

**optional**, **type**: :ref:`metrics name <conf_value_metrics_name>`

Set the global prefix for all metrics.

**default**: "g3proxy"

emit_duration
-------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the emit duration for local stats. All stats will be send out in sequence.

**default**: 200ms
