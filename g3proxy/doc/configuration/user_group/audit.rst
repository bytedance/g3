.. _configuration_user_group_user_audit:

**********
User Audit
**********

.. versionadded:: 1.7.0

The user audit config is in map format. We will use this to specify user level audit actions.

enable_protocol_inspection
--------------------------

**optional**, **type**: bool

Whether protocol inspection functionality should be enabled.

Protocol inspection will be enabled if true, and if audit is also enabled at both server and user side, for a specific user request.

**default**: false

prohibit_unknown_protocol
-------------------------

**optional**, **type**: bool

Whether unknown protocol will be prohibited when protocol inspection is enabled.

**default**: false

application_audit_ratio
-----------------------

**optional**, **type**: :ref:`random ratio <conf_value_random_ratio>`

Set the application audit (like ICAP REQMOD/RESPMOD) ratio for incoming user requests.

This also controls whether protocol inspection is really enabled for a specific user request.

If set, this will override the :ref:`application audit ratio <conf_auditor_application_audit_ratio>` config at auditor side.

**default**: not set

.. versionadded:: 1.7.4
