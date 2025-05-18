.. _configuration_collector:

*********
Collector
*********

The type for each collector config is *map*, with two always required keys:

* :ref:`name <conf_collector_common_name>`, which specify the name of the collector.
* :ref:`type <conf_collector_common_type>`, which specify the real type of the collector, decides how to parse other keys.

There are many types of collector, each with a section below.

Collectors
==========

.. toctree::
   :maxdepth: 1

   aggregate
   discard
   internal
   regulate

Common Keys
===========

This section describes the common keys, they may be used by many collectors.

.. _conf_collector_common_name:

name
----

**required**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

Set the name of the collector.

.. _conf_collector_common_type:

type
----

**required**, **type**: str

Set the type of the collector.

.. _conf_collector_common_next:

next
----

**type**: :ref:`metric node name <conf_value_metric_node_name>`

Set the next collector to use.

If the specified collector doesn't exist in configure, a default Discard collector will be used.

.. _conf_collector_common_exporter:

exporter
--------

**type**: :ref:`metric node name <conf_value_metric_node_name>` | seq

Set the exporter(s) to use.

If the specified exporter doesn't exist in configure, a default Discard exporter will be used.
