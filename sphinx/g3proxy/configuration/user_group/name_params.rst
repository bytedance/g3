.. _config_auth_username_params:

Username Params
===============

The username params can be used to extract egress path selection upstream addr, see :ref:`upstream addr <proto_egress_path_selection_egress_upstream>`.

The config value should be a map, the keys are:

keys_for_host
-------------

**optional**, **type**: list of string

Ordered keys that will be used to form the host label

resolve_sticky_key
------------------

**optional**, **type**: string

The key of the param whose value will be used as the hash key when resolving the upstream domain.

Ketama consistent hash will be used if this is set and the corresponding value can be found in the input username params.

**default**: not set

require_hierarchy
-----------------

**optional**, **type**: bool

Require that if a later key appears, all its ancestors (earlier keys) must also appear

**default**: true

floating_keys
-------------

**optional**, **type**: list of string

Keys that can appear independently without requiring earlier keys (e.g., a generic optional key)

reject_unknown_keys
-------------------

**optional**, **type**: bool

Reject unknown keys not present in `keys_for_host`

**default**: true

reject_duplicate_keys
---------------------

**optional**, **type**: bool

Reject duplicate keys

**default**: true

separator
---------

**optional**, **type**: string

Separator used between labels

**default**: "-"

domain_suffix
-------------

**optional**, **type**: string

Optional domain suffix appended to computed host (e.g., ".svc.local")

**default**: not set

http_port
---------

**optional**: **type**: u16

Default port for HTTP proxy upstream selection

**default**: 10000

socks5_port
-----------

**optional**: **type**: u16

Default port for Socks5 proxy upstream selection

**default**: 10000

strip_suffix_for_auth
---------------------

**optional**, **type**: bool

If true, only the base part before '+' is used for auth username

**default**: true

.. versionadded:: 1.13.0
