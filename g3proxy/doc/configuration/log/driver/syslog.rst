.. _configuration_log_driver_syslog:

syslog
======

The syslog driver config is is map format.

We can set it to send logs to syslogd, which can be listening on

 * unix socket, which is default
 * udp socket

The message format can be

 * rfc3164, which is default
 * rfc5424

The keys are described below.

target_unix
-----------

**optional**, **type**: mix

You can set this if you want to send syslog to a custom unix socket path.

The value can be a map, with the following keys:

* path

  **required**, **type**: :ref:`absolute path <conf_value_absolute_path>`

  The syslogd daemon listen socket path.

If the value type is str, the value should be the same as the value as *path* above.

**default**: not set

target_udp
----------

**optional**, **type**: mix

You can set this if you want to send syslog to a remote syslogd which listening on a udp socket.

The value can be a map, with the following keys:

* address

  **required**, **type**: :ref:`sockaddr str <conf_value_sockaddr_str>`

  Set the remote socket address.

* bind_ip

  **optional**, **type**: :ref:`ip addr str <conf_value_ip_addr_str>`

  Set the ip address to bind to for the local socket.

  **default**: not set

If the value type is str, the value should be the same as the value as *address* above.

**default**: not set

target
------

**optional**, **type**: map

This is just another form to set syslog target address.

The key *udp* is just handled as *target_udp* as above.

The key *unix* is just handled as *target_unix* as above.

.. versionadded:: 1.3.5

format_rfc5424
--------------

**optional**, **type**: mix

Set this to use rfc5424 message format.

The value can be a map, with the following keys:

* enterprise_id

  **optional**, **type**: int

  Set the enterprise id value as described in `rfc5424`_.

  See `PRIVATE ENTERPRISE NUMBERS`_ for IANA allocated numbers.

  **default**: 0, which is reserved

  .. _rfc5424: https://tools.ietf.org/html/rfc5424
  .. _PRIVATE ENTERPRISE NUMBERS: https://www.iana.org/assignments/enterprise-numbers/enterprise-numbers

* message_id

  **optional**, **type**: str

  Set the message id.

  **default**: not set

If the value type is int, the value should be the same as the value as *enterprise_id* above.
If the value type is str, the value should be the same as the value as *message_id* above.

**default**: not set

use_cee_log_syntax
------------------

**optional**, **type**: bool

Set if we should use `CEE Log Syntax`_.

Enable this option if you need to use rsyslog `mmjsonparse`_ module.

**default**: not set

.. _mmjsonparse: https://www.rsyslog.com/files/temp/doc-indent/configuration/modules/mmjsonparse.html
.. _CEE Log Syntax: https://cee.mitre.org/language/1.0-beta1/cls.html

cee_event_flag
--------------

**optional**, **type**: ascii string

Set a custom CEE event flag value. Only meaningful if *use_cee_log_syntax* is set.

The one defined by `CLT`_ is *@cee:*, you can override it by using this option.

**default**: @cee:

.. _CLT: https://cee.mitre.org/language/1.0-beta1/clt.html

emit_hostname
-------------

**optional**, **type**: bool

Set if we should set hostname in the syslog message header.

**default**: false

.. versionadded:: 1.5.4

append_report_ts
----------------

**optional**, **type**: bool

Set if we should add :ref:`report_ts <log_shared_keys_report_ts>` to logs.

**default**: false
