.. _configuration_escaper_route_mapping:

route_mapping
=============

This escaper allows to select a next escaper based on the user specified path selection index.

If no index can be get from the path selection method, the default random one will be used.

No common keys are supported.

next
----

**required**, **type**: seq

This set all the next escapers. Each element should be the name of the target float escaper.

.. note:: No duplication of next escapers is allowed.
