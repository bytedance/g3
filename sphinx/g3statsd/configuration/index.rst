.. _configuration:

#############
Configuration
#############

YAML is used as the configuration file format. The main conf file,
which should be specified with the command line option *-c*,
is make up of the following entries:

+-------------+----------+-------+------------------------------------------------+
|Key          |Type      |Reload |Description                                     |
+=============+==========+=======+================================================+
|runtime      |Map       |no     |Runtime config, see :doc:`runtime`              |
+-------------+----------+-------+------------------------------------------------+
|worker       |Map [#w]_ |no     |An unaided runtime will be started if present.  |
+-------------+----------+-------+------------------------------------------------+
|controller   |Seq       |no     |Controller config                               |
+-------------+----------+-------+------------------------------------------------+
|importer     |Mix [#m]_ |yes    |Importer config                                 |
+-------------+----------+-------+------------------------------------------------+
|collector    |Mix [#m]_ |yes    |Collector config                                |
+-------------+----------+-------+------------------------------------------------+
|exporter     |Mix [#m]_ |yes    |Exporter config                                 |
+-------------+----------+-------+------------------------------------------------+

.. rubric:: Footnotes

.. [#m] See :ref:`hybrid map <conf_value_hybrid_map>` for the real format.
.. [#w] See :ref:`unaided runtime config <conf_value_unaided_runtime_config>`.

.. toctree::
   :hidden:

   runtime
   importer/index
   collector/index
   exporter/index
   values/index
