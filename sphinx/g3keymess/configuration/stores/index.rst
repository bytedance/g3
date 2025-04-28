.. _configuration_store:

*****
Store
*****

Set the key store.

The type for each store config is *map*, with two always required keys:

* :ref:`name <conf_store_common_name>`, which specify the name of the store.
* :ref:`type <conf_store_common_type>`, which specify the real type of the store, decides how to parse other keys.

There are many types of store, each with a section below.

Stores
======

.. toctree::
   :maxdepth: 2

   local

Common Keys
===========

This section describes the common keys, they may be used by many stores.

.. _conf_store_common_name:

**required**, **type**: :ref:`metrics name <conf_value_metrics_name>`

Set the name of the discover.

.. _conf_store_common_type:

**required**, **type**: str

Set the type of the discover.
