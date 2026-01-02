.. _configuration_auth_user_source:

******
Source
******

Source defines where we can get the config of dynamic users.

The source config is in *map* format, with one required key:

* :ref:`type <conf_auth_user_source_type>`, which specify the type of the source, decides how to parse other keys.

Sources
=======

.. toctree::
   :maxdepth: 1

   file
   lua
   python

Common Keys
===========

This section describes the common keys, they may be used by many sources.

.. _conf_auth_user_source_type:

type
----

**required**, **type**: str

Set the type of the source.
