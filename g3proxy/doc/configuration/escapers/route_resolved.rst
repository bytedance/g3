.. _configuration_escaper_route_resolved:

route_resolved
==============

This escaper allows to select a next escaper based on rules on the resolved upstream ip address.

There is no path selection support for this escaper.

The resolve method in Happy Eyeballs algorithm is used.

The following common keys are supported:

* :ref:`resolver <conf_escaper_common_resolver>`, **required**
* :ref:`resolve_strategy <conf_escaper_common_resolve_strategy>`
* :ref:`default_next <conf_escaper_common_default_next>`

lpm_match
---------

**optional**, **type**: seq

If the resolved upstream ip address lpm match the network in the rules, that escaper will be selected.

Each rule is in *map* format, with two keys:

* next

  **required**, **type**: str

  Set the next escaper.

* networks

  **optional**, **type**: seq

  Each element should be valid network string. Both IPv4 and IPv6 are supported.

  Each network should not be set for different next escapers.

resolution_delay
----------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

The resolution delay time for the wait of the preferred address family after another one is returned.

The meaning is the same as *resolution_delay* field in :ref:`happy eyeballs <conf_value_happy_eyeballs>`.

**default**: 50ms

.. versionadded:: 1.5.5
