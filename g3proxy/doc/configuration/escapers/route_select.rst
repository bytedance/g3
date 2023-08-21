.. _configuration_escaper_route_select:

route_select
============

This escaper allows to select a next escaper based on the specified pick policy.

The following egress path selection methods is supported:

* :ref:`by json <proto_egress_path_selection_by_json>`

  The json value will be parsed as :ref:`next_nodes <conf_escaper_route_select_next_nodes>` as below.
  The select policy can only be set by :ref:`next_pick_policy <conf_escaper_route_select_next_pick_policy>`.

  .. versionadded:: 1.7.22

No common keys are supported.

.. _conf_escaper_route_select_next_nodes:

next_nodes
----------

**required**, **type**: string | seq

Set the next escaper(s) those can be selected.

For *seq* value, each of its element must be :ref:`weighted name str <conf_value_weighted_name_str>`.

.. _conf_escaper_route_select_next_pick_policy:

next_pick_policy
----------------

**optional**, **type**: :ref:`selective pick policy <conf_value_selective_pick_policy>`

Set the policy to select next proxy address.

The key for rendezvous/jump hash is *<client-ip>[-<username>]-<upstream-host>*.

**default**: rendezvous
