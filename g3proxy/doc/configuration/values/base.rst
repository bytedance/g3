
.. _configure_base_value_types:

****
Base
****

.. _conf_value_nonzero_u32:

nonzero u32
===========

**yaml value**: int

A non-zero u32 in value type.

.. _conf_value_humanize_usize:

humanize usize
==============

**yaml value**: int | str

For *str* value, it support units of 2^10 like "KiB", "MiB", or units of 1000 like "KB", "MB".

For *int* value or *str* value without unit, the unit will be bytes.

.. seealso::

   `humanize_rs bytes <https://docs.rs/humanize-rs/0.1.5/humanize_rs/bytes/index.html>`_

.. _conf_value_humanize_u32:

humanize u32
============

**yaml value**: int | str

For *str* value, it support units of 2^10 like "KiB", "MiB", or units of 1000 like "KB", "MB".

For *int* value or *str* value without unit, the unit will be bytes.

.. _conf_value_humanize_duration:

humanize duration
=================

**yaml value**: int | str

For *str* value, at least one unit is required. Multiple units string like "1h 30m 71s" is also supported.
See `duration units`_ for all supported units.

For *int* and *real* value, the unit will be seconds.

.. seealso::

   `humanize_rs duration <https://docs.rs/humanize-rs/0.1.5/humanize_rs/duration/index.html>`_

.. _duration units: https://docs.rs/humanize-rs/0.1.5/src/humanize_rs/duration/mod.rs.html#115

.. _conf_value_username:

username
========

**yaml value**: str

The UTF-8 username to be used in different contexts.
Should be less than or equal to 255 bytes.

.. _conf_value_password:

password
========

**yaml value**: str

The UTF-8 password to be used in different contexts.
Should be less than or equal to 255 bytes.

.. _conf_value_upstream_str:

upstream str
============

**yaml value**: str

The string should be in *<ip>[:<port>]* or *<domain>[:<port>]* format.

If omitted, the *port* will be set to *0*.

.. _conf_value_url_str:

url str
=======

**yaml value**: str

The string should be a valid url.

.. _conf_value_ascii_str:

ascii str
=========

**yaml value**: str

The string should only consists of ascii characters.

.. _conf_value_rfc3339_datetime_str:

rfc3339 datetime str
====================

**yaml value**: str

The string should be a value rfc3339 datetime string.

.. _conf_value_selective_pick_policy:

selective pick policy
=====================

**yaml value**: str

The policy to select item from selective vectors.

The following values are supported:

* random

  The default one.

* serial | sequence

  For nodes with the same weights, the order is kept as in the config.

* round_robin | rr

  For nodes with the same weights, the order is kept as in the config.

* rendezvous

  Rendezvous Hash. The key format is defined in the context of each selective vector.

* jump_hash

  Jump Consistent Hash. The key format is defined in the context of each selective vector.

.. _conf_value_weighted_upstream_addr:

weighted upstream addr
======================

**yaml value**: map | string

A upstream str with weight set, which make can be grouped into selective vector.

The map consists 2 fields:

* addr

  **required**, **type**: :ref:`upstream str <conf_value_upstream_str>`

  The real value.

* weight

  **optional**, **type**: f64

  The weight of the real value.

  **default**: 1.0

If the value type is string, then it's value will be the *addr* field, with *weight* set to default value.

.. _conf_value_weighted_name_str:

weighted name str
=================

**yaml value**: map | string

A name string with weight set, which make can be grouped into selective vector.

The map consists 2 fields:

* name

  **required**, **type**: string

  The name. The meaning of the name is depending on the config context.

* weight

  **optional**, **type**: f64

  The weight of the name.

  **default**: 1.0

If the value type is string, then it's value will be the *name* field, with *weight* set to default value.

.. _conf_value_list:

list
====

**yaml value**: mix

A list container type for type T.

The value could be a single value of type T, or a sequence of values of type T.
