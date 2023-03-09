.. _configuration_user_group_user_site:

*********
User Site
*********

.. versionadded:: 1.3.4

The user site config is in map format. We can set how to match this site, enable site level metrics, or do any other
site level config.

.. _conf_user_group_user_site_id:

id
--

**required**, **type**: :ref:`metrics name <conf_value_metrics_name>`

Each site should have an ID, and it will be used in metrics name if enabled.

exact_match
-----------

**optional**, **type**: :ref:`host <conf_value_host>`

Set the exact domain or the exact target IP in user request which we should match.

.. note:: the value should be different within all sites config of the current user.

child_match
-----------

**optional**, **type**: :ref:`domain <conf_value_domain>`

Set the parent domain and all it's child domains will be matched.

.. note:: the value should be different within all sites config of the current user.

subnet_match
------------

**optional**, **type**: :ref:`ip network str <conf_value_ip_network_str>`

Set the network to match if the target is IP address in user request.

.. note:: the value should be different within all sites config of the current user.

emit_stats
----------

**optional**, **type**: bool

Set whether we should emit site level stats for this site.

See :ref:`user site metrics <metrics_user_site>` for the definition of metrics.

**default**: false

resolve_strategy
----------------

**optional**, **type**: :ref:`resolve strategy <conf_value_resolve_strategy>`

Set a custom resolve strategy at user-site level, which will override the one at user level,
but still within the range of the one set on the escaper.
Not all escapers support this, see the documentation for each escaper for more info.

**default**: not custom resolve strategy is set

.. versionadded:: 1.7.10
