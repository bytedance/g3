.. _configure_base_value_types:

****
Base
****

.. _conf_value_env_var:

env var
=======

**yaml value**: str

Set a environment variable, in the form '$' + variable name, E.g. $TCP_LISTEN_ADDR.

The value of the environment variable will be parsed just as you write this value as *yaml string* directly there.

.. _conf_value_humanize_usize:

humanize usize
==============

**yaml value**: int | str

For *str* value, it support units of 2^10 like "KiB", "MiB", or units of 1000 like "KB", "MB".

For *int* value or *str* value without unit, the unit will be bytes.

.. seealso::

   `humanize_rs bytes <https://docs.rs/humanize-rs/0.1.5/humanize_rs/bytes/index.html>`_

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

.. _conf_value_upstream_str:

upstream str
============

**yaml value**: str

The string should be in *<ip>[:<port>]* or *<domain>[:<port>]* format.

If omitted, the *port* will be set to *0*.
