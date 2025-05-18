.. _configure_network_value_types:

*******
Network
*******

.. _conf_value_sockaddr_str:

sockaddr str
============

**yaml value**: str

The string should be in *<ip>[:<port>]* format, in which the port may be omitted if a default value is available.

.. _conf_value_static_sockaddr_str:

static sockaddr str
===================

**yaml value**: str

The string should be in *@<domain>:<port>* or *@<ip>:<port>* format.

It is different from :ref:`upstream str <conf_value_upstream_str>` as:

- It will be resolved when we load the config files
- The domain is only allowed to be resolved to just 1 IP address

.. _conf_value_env_sockaddr_str:

env sockaddr str
================

**yaml value**: :ref:`sockaddr str <conf_value_sockaddr_str>` or :ref:`static sockaddr str <conf_value_static_sockaddr_str>` or :ref:`env var <conf_value_env_var>`

The string should be in *<ip>[:<port>]* format, in which the port may be omitted if a default value is available.

.. _conf_value_ip_addr_str:

ip addr str
===========

**yaml value**: str

The string should be in *<ip>* format.

.. _conf_value_interface_name:

interface name
==============

**yaml value**: str | u32

The string should be a network interface name or index.

.. _conf_value_host:

host
====

**yaml value**: str

A host value. Which should be either a valid domain, or a valid IP address.

.. _conf_value_tcp_listen:

tcp listen
==========

**yaml value**: mix

It consists of the following fields:

* address

  **required**, **type**: :ref:`env sockaddr str <conf_value_env_sockaddr_str>`

  Set the listen socket address.

  **default**: [::]:0, which has empty port

* interface

  **optional**: **type**: :ref:`interface name <conf_value_interface_name>`

  Bind the outgoing socket to a particular device like “eth0”.

  **default**: not set

  .. versionadded:: 0.4.2

* backlog

  **optional**, **type**: unsigned int

  Set the listen backlog number for tcp sockets. The default value will be used if the specified value is less than 8.

  **default**: 4096

  .. note::

    If the backlog argument is greater than the value in /proc/sys/net/core/somaxconn, then it is silently truncated
    to that value. Since Linux 5.4, the default in this file is 4096; in earlier kernels, the default value is 128.

* netfilter_mark

  **optional**, **type**: unsigned int

  Set the netfilter mark (SOL_SOCKET, SO_MARK) value for the listening socket. If this field not present,
  the mark value will not be touch. This value can be used for advanced routing policy or netfilter rules.

* ipv6_only

  **optional**, **type**: bool

  Listen only to ipv6 address only if address is set to [::].

  **default**: false

* instance

  **optional**, **type**: int

  Set how many listen instances. If *scale* is set, this will be the least value.

  **default**: 1

* scale

  **optional**, **type**: float | string

  Set the listen instance count scaled according to available parallelism.

  For string value, it could be in percentage (n%) or fractional (n/d) format.

  Example:

  .. code-block:: yaml

    scale: 1/2
    # or
    scale: 0.5
    # or
    scale: 50%

  **default**: 0

* follow_cpu_affinity

  **optional**, **type**: bool

  Follow CPU affinity of the listen socket and the worker.

  When enabled, it will:

  - when listen in worker

    it will set the following options for the listen socket:

    - Linux: set SO_INCOMING_CPU to the CPU core ID if the worker bind to a specific CPU core
    - FreeBSD: set TCP_REUSPORT_LB_NUMA to TCP_REUSPORT_LB_NUMA_CURDOM if the worker has CPU affinity settings

  - when not listen in worker

    - Linux: get the SO_INCOMING_CPU value of the accepted socket and select a worker run only on that CPU core

  **default**: false

  .. versionadded:: 0.3.8

The yaml value for *listen* can be in the following formats:

* int

  Set the port only.

* :ref:`sockaddr str <conf_value_sockaddr_str>`

  Set ip and port. The port field is required.

* map

  The keys of this map are the fields as described above.

.. _conf_value_tcp_keepalive:

tcp keepalive
=============

**yaml value**: mix

This set TCP level keepalive settings.

It consists of 2 fields:

* enable

  **optional**, **type**: bool

  Set whether tcp keepalive should be enabled.

  **default**: false, which means you can set limit on other values in case keepalive is needed somewhere

* idle_time

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the keepalive idle time.

  **default**: 60s

* probe_interval

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the probe interval after idle.

  **default**: not set, which means the OS default value will be used

* probe_count

  **optional**, **type**: u32

  Set the probe count.

  **default**: not set, which means the OS default value will be used

If the root value type is bool, the value will be parsed the same as the *enable* key.

If the root value type is not map and not bool, the value will be parsed the same as the *idle_time* key, but with
*enable* set to true.
