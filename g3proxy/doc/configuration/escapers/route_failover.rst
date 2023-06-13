.. _configuration_escaper_route_failover:

route_failover
==============

.. versionadded:: 1.7.17

This escaper allows to failover between the primary and standby next escaper.

.. note:: The http forward capability will be downgraded for unmatched next escapers.

There is no path selection support for this escaper.

No common keys are supported.

primary_next
------------

**required**, **type**: str

Set the primary next escaper to be used.

standby_next
------------

**required**, **type**: str

Set the standby next escaper to be used.

fallback_delay
--------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the delay time that we should wait before using the standby escaper while stilling waiting for response
from the primary escaper.

**default**: 100ms
