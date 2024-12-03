.. _configuration_escaper_route_client:

route_client
============

.. versionadded:: 1.1.3

This escaper allows to select a next escaper based on rules on client address.

There is no path selection support for this escaper.

The following common keys are supported:

* :ref:`default_next <conf_escaper_common_default_next>`

exact_match
-----------

**optional**, **type**: seq

If the client ip exactly match the one in the rules, that escaper will be selected.

Each rule is in *map* format, with two keys:

* next

  **required**, **type**: str

  Set the next escaper.

* ips

  **optional**, **type**: seq

  Each element should be :ref:`ip addr str <conf_value_ip_addr_str>`.

  An ip should not be set duplicated in rules for different next escapers.

subnet_match
------------

**optional**, **type**: seq

If the client ip match the longest subnet in the rule, that escaper will be selected.

Each rule is in *map* format, with two keys:

* next

  **required**, **type**: str

  Set the next escaper.

* subnets

  **optional**, **type**: seq

  Each element should be :ref:`ip network str <conf_value_ip_network_str>`.

  A subnet should not be set duplicated in rules for different next escapers.
