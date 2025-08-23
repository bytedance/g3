.. _configuration_escaper_route_mapping:

route_mapping
=============

This escaper allows to select a next escaper based on the user specified path selection index.

The following egress path selection methods is supported:

* :ref:`by index <proto_egress_path_selection_by_index>`

  The index will be used as the index of the next escaper

  If no index can be get from the path selection method, the default random one will be used.

No common keys are supported.

next
----

**required**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

This set all the next escapers. Each element should be the name of the target float escaper.

.. note:: No duplication of next escapers is allowed.
