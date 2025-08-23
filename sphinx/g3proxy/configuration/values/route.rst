.. _configure_route_value_types:

*****
Route
*****

.. _conf_value_host_matched_object:

Host Matched Object
===================

**yaml value**: map | seq of map

This set a host based match object for generic type T, which will be specified in the real config options.

The yaml value for T will be a map, but with the following keys as reserved as they are used by the match rules:

* exact_match

  **optional**, **type**: :ref:`host <conf_value_host>`

  Match if this is the exact host.

* child_match

  **optional**, **type**: :ref:`domain <conf_value_domain>`

  Match if the target host is a child domain of this parent domain.

* set_default

  **optional**, **type**: bool

  If true, also set this T as default value

  **default**: false

If none of the above keys found, the parsed T value will also be used as the default value.

A match object can contains one or more T(s), which means the yaml type for this object could be a single T,
or a sequence of T.

Only a single T is allowed for each match rules, including the default one.

.. _conf_value_uri_path_matched_object:

Uri Path Matched Object
=======================

**yaml value**: map | seq of map

This set a url path based match object for generic type T, which will be specified in the real config options.

The yaml value for T will be a map, but with the following keys as reserved as they are used by the match rules:

* prefix_match

  **optional**, **type**: str

  Match if the target url path has this prefix.

* set_default

  **optional**, **type**: bool

  If true, also set this T as default value

  **default**: false

If none of the above keys found, the parsed T value will also be used as the default value.

A match object can contains one or more T(s), which means the yaml type for this object could be a single T,
or a sequence of T.

Only a single T is allowed for each match rules, including the default one.
