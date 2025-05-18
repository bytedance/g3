********
Importer
********

The type for each importer config is *map*, with two always required keys:

* :ref:`name <conf_importer_common_name>`, which specify the name of the importer.
* :ref:`type <conf_importer_common_type>`, which specify the real type of the importer, decides how to parse other keys.

There are many types of importer, each with a section below.

Importers
=========

.. toctree::
   :maxdepth: 1

   dummy
   statsd

Common Keys
===========

This section describes the common keys, they may be used by many importers.

.. _conf_importer_common_name:

name
----

**required**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

Set the name of the importer.

.. _conf_importer_common_type:

type
----

**required**, **type**: str

Set the type of the importer.

.. _conf_importer_common_collector:

collector
---------

**type**: :ref:`metric node name <conf_value_metric_node_name>`

Set the collector to use for this importer.

If the specified collector doesn't exist in configure, a default Discard collector will be used.

.. _conf_importer_common_listen_in_worker:

listen_in_worker
----------------

**optional**, **type**: bool

Set if we should listen in each worker runtime if you have worker enabled.

The listen instance count will be the same with the worker number count.

**default**: false

.. _conf_importer_common_ingress_network_filter:

ingress_network_filter
----------------------

**optional**, **type**: :ref:`ingress network acl rule <conf_value_ingress_network_acl_rule>`

Set the network filter for clients.

The used client address will always be the interpreted client address, which means it will be the raw socket peer addr
for servers that listen directly, and it will be the address set in the PROXY Protocol message for serverw chained after
the server that support PROXY Protocol.

**default**: not set
