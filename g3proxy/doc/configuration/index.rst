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
|group_name |Str       |no     |Process group name, default to be empty, can be |
|           |          |       |overridden by the *-G* command line option.     |
+-----------+----------+-------+------------------------------------------------+
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
|user_group |Mix [#m]_ |yes    |User group config, see :doc:`user_group/index`  |
+-----------+----------+-------+------------------------------------------------+
|auditor    |Mix [#m]_ |yes    |Auditor config, see :doc:`auditors/index`       |
+-----------+----------+-------+------------------------------------------------+
|server     |Mix [#m]_ |yes    |Server config, see :doc:`servers/index`         |
+-----------+----------+-------+------------------------------------------------+

Example config: :doc:`example config for rd-relay service <example>`

.. rubric:: Footnotes

.. [#m] *Mix* is not a yaml type, see :ref:`hybrid map <conf_value_hybrid_map>` for the real format.
.. [#w] See :ref:`unaided runtime config <conf_value_unaided_runtime_config>`.

.. toctree::
   :hidden:

   values/index
   runtime
   log/index
   stat
   resolvers/index
   escapers/index
   auditors/index
   user_group/index
   servers/index
   example

