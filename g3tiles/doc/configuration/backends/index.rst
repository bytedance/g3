.. _configuration_backend:

*******
Backend
*******

The type for each backend config is *map*, with two always required keys:

* :ref:`name <conf_backend_common_name>`, which specify the name of the backend.
* :ref:`type <conf_backend_common_type>`, which specify the real type of the backend, decides how to parse other keys.

There are many types of backend, each with a section below.

Backends
========

.. toctree::
   :maxdepth: 2

   dummy_close
   keyless_quic
   keyless_tcp
   stream_tcp

Common Keys
===========

This section describes the common keys, they may be used by many backends.

.. _conf_backend_common_name:

**required**, **type**: :ref:`metrics name <conf_value_metrics_name>`

Set the name of the backend.

.. _conf_backend_common_type:

**required**, **type**: str

Set the type of the backend.

.. _conf_backend_common_discover:

discover
--------

**required**, **type**: :ref:`metrics name <conf_value_metrics_name>`

Set the discover that this backend should use.

.. _conf_backend_common_discover_data:

discover_data
-------------

**required**, **type**: :ref:`discover register data <conf_discover_register_data>`

Set the data that will be registered to :ref:`discover <conf_backend_common_discover>`.

.. _conf_backend_common_extra_metrics_tags:

extra_metrics_tags
------------------

**optional**, **type**: :ref:`static metrics tags <conf_value_static_metrics_tags>`

Set extra metrics tags that should be added to backend stats.

**default**: not set
