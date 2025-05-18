
.. _configure_acl_value_types:

***
ACL
***

All acl value types are described here.

Basic Type
==========

.. _conf_value_acl_action:

acl action
----------

**yaml value**: str

There are 4 types of acl actions:

* forbid_log

  Forbid if match the rule and log. Alternatives: deny_log, reject_log.

* forbid

  Forbid if match the rule. Alternatives: deny, reject.

* permit_log

  Permit if match the rule and log. Alternatives: allow_log, accept_log.

* permit

  Permit if match the rule. Alternatives: allow, accept.

The match order is the same as the list order above.

.. _conf_value_acl_rule:

acl rule
--------

**yaml value**: mix

All the rules share the same config format described in this section.

An acl rule is consisted of many records, each of them has an associated :ref:`acl action <conf_value_acl_action>`.
A default missed action can be set in the rule, it set the default action if no record matches.

The value in map format is consisted of the following fields:

* default

  Set the default acl action if no rule match.

  Default action if rule is set but with *default* omitted: forbid if not specified in the rule's doc.

* any of the acl actions as the key str

  The value should be a valid record or a list of them, with the key string as the acl action.
  See detail types for the format of each record type.

The value could also be a single record or a list of them, which means only them are permitted with no log.

The default missed action is **forbid** and the default found action is **permit**,
if they are not specified in the detail types.

Detail Type
===========

.. _conf_value_network_acl_rule:

network acl rule
----------------

**yaml value**: :ref:`acl rule <conf_value_acl_rule>`

The record type should be :ref:`ip network str <conf_value_ip_network_str>`.

.. _conf_value_ingress_network_acl_rule:

ingress network acl rule
------------------------

**yaml value**: :ref:`network acl rule <conf_value_network_acl_rule>`

The same type as network acl rule. Default added: permit 127.0.0.1 and ::1.
