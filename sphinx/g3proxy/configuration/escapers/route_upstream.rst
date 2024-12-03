.. _configuration_escaper_route_upstream:

route_upstream
==============

This escaper allows to select a next escaper based on rules on upstream address.

There is no path selection support for this escaper.

The following common keys are supported:

* :ref:`default_next <conf_escaper_common_default_next>`

exact_match
-----------

**optional**, **type**: seq

If the host part of upstream address exactly match the one in the rules, that escaper will be selected.

Each rule is in *map* format, with two keys:

* next

  **required**, **type**: str

  Set the next escaper.

* hosts

  **optional**, **type**: seq

  Each element should be :ref:`host <conf_value_host>`.

  A host should not be set duplicated in rules for different next escapers.

subnet_match
------------

**optional**, **type**: seq

If the host is an IP address and match the longest subnet in the rule, that escaper will be selected.

Each rule is in *map* format, with two keys:

* next

  **required**, **type**: str

  Set the next escaper.

* subnets

  **optional**, **type**: seq

  Each element should be :ref:`ip network str <conf_value_ip_network_str>`.

  A subnet should not be set duplicated in rules for different next escapers.

child_match
-----------

**optional**, **type**: seq

If the domain of the upstream address is children of domains in the rules, that escaper will be selected.

Each rule is in *map* format, with two keys:

* next

  **required**, **type**: str

  Set the next escaper.

* domains

  **optional**, **type**: seq

  Each element should be :ref:`domain <conf_value_domain>`.

  Each domain should not be set for different next escapers.

radix_match
-----------

**optional**, **type**: seq

If the domain of the upstream address exactly match the one of the domain suffixes in the rules,
that escaper will be selected.

Each rule is in *map* format, with two keys:

* next

  **required**, **type**: str

  Set the next escaper.

* suffixes

  **optional**, **type**: seq

  Each element should be :ref:`domain <conf_value_domain>`.

  Each domain suffix should not be set for different next escapers.
