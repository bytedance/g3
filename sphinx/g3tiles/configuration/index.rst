.. _configuration:

#############
Configuration
#############

YAML is used as the configuration file format. The main conf file,
which should be specified with the command line option *-c*,
is make up of the following entries:

+-----------+----------+-------+------------------------------------------------+
|Key        |Type      |Reload |Description                                     |
+===========+==========+=======+================================================+
|runtime    |Map       |no     |Runtime config, see :doc:`runtime`              |
+-----------+----------+-------+------------------------------------------------+
|worker     |Map [#w]_ |no     |An unaided runtime will be started if present.  |
+-----------+----------+-------+------------------------------------------------+
|log        |Map       |no     |Log config, see :doc:`log/index`                |
+-----------+----------+-------+------------------------------------------------+
|stat       |Map       |no     |Stat config, see :doc:`stat`                    |
+-----------+----------+-------+------------------------------------------------+
|controller |Seq       |no     |Controller config                               |
+-----------+----------+-------+------------------------------------------------+
|discover   |Mix [#m]_ |yes    |Discover config                                 |
+-----------+----------+-------+------------------------------------------------+
|backend    |Mix [#m]_ |yes    |Backend config                                  |
+-----------+----------+-------+------------------------------------------------+
|server     |Mix [#m]_ |yes    |Server config, see :doc:`servers/index`         |
+-----------+----------+-------+------------------------------------------------+

.. rubric:: Footnotes

.. [#m] See :ref:`hybrid map <conf_value_hybrid_map>` for the real format.
.. [#w] See :ref:`unaided runtime config <conf_value_unaided_runtime_config>`.

.. toctree::
   :hidden:

   runtime
   log/index
   stat
   discovers/index
   backends/index
   servers/index
   values/index
