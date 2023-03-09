.. _configuration_escaper_route_select:

route_select
============

This escaper allows to select a next escaper based on the specified pick policy.

There is no path selection support for this escaper.

No common keys are supported.

next_nodes
----------

**required**, **type**: string | seq

Set the next escaper(s) those can be selected.

For *seq* value, each of its element must be :ref:`weighted name str <conf_value_weighted_name_str>`.

next_pick_policy
----------------

**optional**, **type**: :ref:`selective pick policy <conf_value_selective_pick_policy>`

Set the policy to select next proxy address.

The key for rendezvous/jump hash is *<client-ip>[-<username>]-<upstream-host>*.

**default**: rendezvous
