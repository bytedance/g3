.. _configuration_escaper_route_select:

route_select
============

This escaper allows to select a next escaper based on the specified pick policy.

The following egress path selection methods is supported:

* :ref:`by id map <proto_egress_path_selection_by_id_map>`

  If matched, an escaper registered in :ref:`next_nodes <conf_escaper_route_select_next_nodes>` which
  the name is the same with `ID` will be used.

  The escaper with name `ID` must be present in :ref:`next_nodes <conf_escaper_route_select_next_nodes>`.
  You can set the weight to 0 to avoid a default selection.

  .. versionadded:: 1.7.22

No common keys are supported.

.. _conf_escaper_route_select_next_nodes:

next_nodes
----------

**required**, **type**: :ref:`weighted metric node name <conf_value_weighted_metric_node_name>` | seq

Set the next escaper(s) those can be selected.

.. _conf_escaper_route_select_next_pick_policy:

next_pick_policy
----------------

**optional**, **type**: :ref:`selective pick policy <conf_value_selective_pick_policy>`

Set the policy to select next proxy address.

The key for ketama/rendezvous/jump hash is *<client-ip>[-<username>]-<upstream-host>*.

**default**: ketama
