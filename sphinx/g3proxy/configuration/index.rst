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
|resolver   |Mix [#m]_ |yes    |Resolver config, see :doc:`resolvers/index`     |
+-----------+----------+-------+------------------------------------------------+
|escaper    |Mix [#m]_ |yes    |Escaper config, see :doc:`escapers/index`       |
+-----------+----------+-------+------------------------------------------------+
|user_group |Mix [#m]_ |yes    |User group config, see :doc:`auth/index`        |
+-----------+----------+-------+------------------------------------------------+
|auditor    |Mix [#m]_ |yes    |Auditor config, see :doc:`auditors/index`       |
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
   resolvers/index
   escapers/index
   auditors/index
   auth/index
   servers/index
   values/index
