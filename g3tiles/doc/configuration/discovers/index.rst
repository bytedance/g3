.. _configuration_discover:

********
Discover
********

The type for each discover config is *map*, with two always required keys:

* :ref:`name <conf_discover_common_name>`, which specify the name of the server.
* :ref:`type <conf_discover_common_type>`, which specify the real type of the discover, decides how to parse other keys.

There are many types of discover, each with a section below.

Discovers
=========

.. toctree::
   :maxdepth: 2

   static_addr
   host_resolver

Common Keys
===========

This section describes the common keys, they may be used by many discovers.

.. _conf_discover_common_name:

**required**, **type**: :ref:`metrics name <conf_value_metrics_name>`

Set the name of the discover.

.. _conf_discover_common_type:

**required**, **type**: str

Set the type of the discover.

.. _conf_discover_register_data:

Register Data
=============

Each discover will have it's own format for the register data. Follow the link bellow to see more details.

+--------------+----------------------------------------------------------------------+
|Type          |Link                                                                  |
+==============+======================================================================+
|static_addr   |:ref:`static_addr data <conf_discover_static_addr_register_data>`     |
+--------------+----------------------------------------------------------------------+
|host_resolver |:ref:`host_resolver data <conf_discover_host_resolver_register_data>` |
+--------------+----------------------------------------------------------------------+
