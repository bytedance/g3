
.. _configure_resolve_value_types:

*******
Resolve
*******

.. _conf_value_resolve_strategy:

Resolve Strategy
================

**yaml value**: mix

The *Resolve Strategy* config is not for resolvers, but for the users of resolvers.

The type for this is *map*, which consists of keys as follows:

query
-----

**optional**, **type**: enum str

The query strategy, which will be used by the resolver while resolving.

The value should be:

* Ipv4Only
* Ipv6Only
* Ipv4First (default)
* Ipv6First

pick
----

**optional**, **type**: enum str

The pick strategy, which will be used when selecting the best ip address from all the results.

The value should be:

* Random (default)
* First

.. _conf_value_resolve_redirection:

Resolve Redirection
===================

**yaml value**: mix

The *Resolve Redirection* config is not for resolvers, but for the users of resolvers.

The type for this could be *seq*, which consists of many rules of type
:ref:`resolve redirection rule <conf_value_resolve_redirection_rule>`.

The type for this could also be *map*, in such case, each kv pair will be one rule,
with the key as it's *exact* value, and the value as it's *to* value.

.. _conf_value_resolve_redirection_rule:

Resolve Redirection Rule
------------------------

Each rule should be a map with the following keys:

* exact

  **required**: false, **type**: :ref:`domain <conf_value_domain>`

  Set the exact domain to replace.

* parent

  **required**: false, **type**: :ref:`domain <conf_value_domain>`

  Set the parent domain to replace.

* to

  **required**: true, **type**: mix

  Set the replacement value for the match.

  For *exact* match, the value should be :ref:`host <conf_value_host>` or an array of ip addresses.

  For *parent* match, the value should be :ref:`domain <conf_value_domain>`.

Either *exact* or *parent* should be set for the rule.
