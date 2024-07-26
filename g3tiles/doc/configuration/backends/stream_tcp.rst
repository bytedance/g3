.. _configuration_backend_stream_tcp:

**********
stream_tcp
**********

A layer-4 tcp connect backend.

This will only work with stream tasks.

Config Keys
===========

The following common keys are supported:

* :ref:`discover <conf_backend_common_discover>`
* :ref:`discover_data <conf_backend_common_discover_data>`
* :ref:`extra_metrics_tags <conf_backend_common_extra_metrics_tags>`

peer_pick_policy
----------------

**optional**, **type**: :ref:`selective pick policy <conf_value_selective_pick_policy>`

Set the policy to select next peer address.

The key for ketama/rendezvous/jump hash is *<client-ip>*.

**default**: random

duration_stats
--------------

**optional**, **type**: :ref:`histogram metrics <conf_value_histogram_metrics>`

Histogram metrics config for the tcp connect duration stats.

**default**: set with default value
