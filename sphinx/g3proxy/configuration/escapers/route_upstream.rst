.. _configuration_escaper_route_upstream:

route_upstream
==============

This escaper allows to select a next escaper based on rules on upstream address.

There is no path selection support for this escaper.

The following common keys are supported:

* :ref:`default_next <conf_escaper_common_default_next>`

exact_match
-----------

**optional**, **type**: seq | map

If the host part of upstream address exactly match the one in the rules, that escaper will be selected.

For seq format:

  Each rule is in *map* format, with two keys:

  * next

    **required**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

    Set the next escaper.

  * hosts

    **optional**, **type**: seq, **alias**: host

    Each element should be :ref:`host <conf_value_host>`.

    A host should not be set duplicated in rules for different next escapers.

  Example:

  .. code-block:: yaml

    - next: deny
      hosts:
        - example.net
    - next: allow
      hosts:
        - 192.168.1.1

For map format:

  The key should be the next escaper name, and the value should be the same as `hosts` in the seq format.

  Example:

  .. code-block:: yaml

    deny:
      - example.net
    allow:
      - 192.168.1.1

.. versionchanged:: 1.11.5 support map format

subnet_match
------------

**optional**, **type**: seq | map

If the host is an IP address and match the longest subnet in the rule, that escaper will be selected.

For seq format:

  Each rule is in *map* format, with two keys:

  * next

    **required**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

    Set the next escaper.

  * subnets

    **optional**, **type**: seq, **alias**: subnet

    Each element should be :ref:`ip network str <conf_value_ip_network_str>`.

    A subnet should not be set duplicated in rules for different next escapers.

  Example:

  .. code-block:: yaml

    - next: deny
      subnets:
        - 192.168.0.0/16
    - next: allow
      subnets:
        - 192.168.0.0/24

For map format:

  The key should be the next escaper name, and the value should be the same as `subnets` in the seq format.

  Example:

  .. code-block:: yaml

    deny:
      - 192.168.0.0/16
    allow:
      - 192.168.0.0/24

.. versionchanged:: 1.11.5 support map format

child_match
-----------

**optional**, **type**: seq | map

If the domain of the upstream address is children of domains in the rules, that escaper will be selected.

For seq format:

  Each rule is in *map* format, with two keys:

  * next

    **required**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

    Set the next escaper.

  * domains

    **optional**, **type**: seq, **alias**: domain

    Each element should be :ref:`domain <conf_value_domain>`.

    Each domain should not be set for different next escapers.

  Example:

  .. code-block:: yaml

    - next: deny
      domains:
        - example.net
    - next: allow
      domains:
        - test.example.net

For map format:

  The key should be the next escaper name, and the value should be the same as `domains` in the seq format.

  Example:

  .. code-block:: yaml

    deny:
      - example.net
    allow:
      - test.example.net

.. versionchanged:: 1.11.5 support map format

suffix_match
------------

**optional**, **type**: seq | map, **alias**: radix_match

If the domain of the upstream address exactly match the one of the domain suffixes in the rules,
that escaper will be selected.

For seq format:

  Each rule is in *map* format, with two keys:

  * next

    **required**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

    Set the next escaper.

  * suffixes

    **optional**, **type**: seq, **alias**: suffix

    Each element should be :ref:`domain <conf_value_domain>`.

    Each domain suffix should not be set for different next escapers.

  Example:

  .. code-block:: yaml

    - next: deny
      suffixes:
        - example.net
    - next: allow
      suffixes:
        - t.example.net
    # test.example.net will match `allow`

For map format:

  The key should be the next escaper name, and the value should be the same as `suffixes` in the seq format.

  .. code-block:: yaml

    deny:
      - example.net
    allow:
      - t.example.net
    # test.example.net will match `allow`

.. versionchanged:: 1.11.5 support map format

regex_match
-----------

**optional**, **type**: seq | map

If the domain of the upstream address matches the one of the domain regex expressions in the rules,
that escaper will be selected.

For seq format:

  Each rule is in *map* format, with two keys:

  * next

    **required**, **type**: :ref:`metric node name <conf_value_metric_node_name>`

    Set the next escaper.

  * rules

    **optional**, **type**: seq, **alias**: rule

    Each element should be a map or :ref:`regex str <conf_value_regex_str>`.

    The following keys are used in the map format:

      - parent

        **optional**, **type**: :ref:`domain <conf_value_domain>`

        The parent domain to strip out (including '.') before do the regex match check.
        If omitted the full domain will be used.

      - regex

        **required**, **type**: :ref:`regex str <conf_value_regex_str>`

        The regex expression.

    Each rule should not be set for different next escapers.

  Example:

  .. code-block:: yaml

    - next: deny
      rules:
        - parent: example.net
          regex: abc.*  # only match the sub part
    - next: allow
      rules:
        - parent: example.net
          regex: tes.+ # only match the sub part
        - .*[.]example[.]org  # match the full domain
    # test.example.net will match `allow`

For map format:

  The key should be the next escaper name, and the value should be the same as `rules` in the seq format.

  Example:

  .. code-block:: yaml

    deny:
      - parent: example.net
        regex: abc.*  # only match the sub part
    allow:
      - parent: example.net
        regex: tes.+ # only match the sub part
      - .*[.]example[.]org  # match the full domain
    # test.example.net will match `allow`

.. versionadded:: 1.11.5
