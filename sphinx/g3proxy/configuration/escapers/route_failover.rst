.. _configuration_escaper_route_failover:

route_failover
==============

.. versionadded:: 1.7.17

This escaper allows to failover between the primary and standby next escaper.

There are some limitation with this escaper:

 - The http forward capability will be set if both the primary and the standby final escaper support it.
 - The audit settings on the primary next path will always be used. The standby path will be ignored.

There is no path selection support for this escaper.

No common keys are supported.

primary_next
------------

**required**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

Set the primary next escaper to be used.

standby_next
------------

**required**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

Set the standby next escaper to be used.

fallback_delay
--------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the delay time that we should wait before using the standby escaper while stilling waiting for response
from the primary escaper.

**default**: 100ms
