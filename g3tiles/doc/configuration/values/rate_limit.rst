
.. _configure_rate_limit_value_types:

**********
Rate Limit
**********

.. _conf_value_tcp_sock_speed_limit:

tcp socket speed limit
======================

**yaml value**: mix

It consists of 3 fields:

* shift_millis | shift

  **type**: int

  The time slice we use to count is *2 ^ N* milliseconds, where N is set by this key and should be in range [0 - 12].
  If N is 10, and the time slice is 1024ms. If omitted, this means the limit is not set.

* upload | north | upload_bytes | north_bytes

  **type**: :ref:`humanize usize <conf_value_humanize_usize>`

  This sets the upload bytes in the time slice. *0* means delay forever.

* download | south | download_bytes | south_bytes

  **type**: :ref:`humanize usize <conf_value_humanize_usize>`

  This set the max download bytes in the time slice. *0* means delay forever.

The yaml value for *tcp_sock_speed_limit* can be in varies formats:

* :ref:`humanize usize <conf_value_humanize_usize>`

  This will set upload and download to the same value, with shift_millis set to 10.

* map

  The keys of this map are the fields as described above.

.. _conf_value_udp_sock_speed_limit:

udp socket speed limit
======================

**yaml value**: mix

It consists of 4 fields:

* shift_millis | shift

  **type**: int

  The time slice we use to count is *2 ^ N* milliseconds, where N is set by this key and should be in range [0 - 12].
  If N is 10, and the time slice is 1024ms. If omitted, this means the limit is not set.

* upload_bytes | north_bytes

  **type**: :ref:`humanize usize <conf_value_humanize_usize>`

  This sets the upload bytes in the time slice. *0* means no limit.

* download_bytes | south_bytes

  **type**: :ref:`humanize usize <conf_value_humanize_usize>`

  This set the max download bytes in the time slice. *0* means no limit.

* upload_packets | north_packets

  **type**: int [usize]

  This set the max upload packets in the time slice. *0* means no limit.

* download_packets | south_packets

  **type**: int [usize]

  This set the max download packets in the time slice. *0* means no limit.

The yaml value for *udp_sock_speed_limit* can be in varies formats:

* :ref:`humanize usize <conf_value_humanize_usize>`

  This will set upload and download bytes to the same value, set shift_millis to 10 and disable check on packets.

* map

  The keys of this map are the fields as described above.

.. _conf_value_request_limit:

request limit
=============

**yaml value**: mix

It consists of 2 fields:

* shift_millis | shift

  **type**: int

  The time slice we use to count is *2 ^ N* milliseconds, where N is set by this key and should be in range [0 - 12].
  If N is 10, and the time slice is 1024ms. If omitted, this means the limit is not set.

* requests

  **type**: usize

  This sets the max requests in the time slice. 0 is not allowed.

.. _conf_value_rate_limit_quota:

rate limit quota
================

**yaml value**: mix

It consists of 3 fields:

* rate

  **type**: :ref:`nonzero u32 <conf_value_nonzero_u32>`

  If int or str without any unit, the default unit will be per second.

  Supported units for str:

    - /s, per second
    - /m, per minute
    - /h, per hour

* replenish_interval

  **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Construct a quota that replenishes one cell in a given interval. The default max_burst value is 1 is its not specified
  along with this option.

* max_burst

  Adjusts the maximum burst size for a quota to construct a rate limiter with a capacity
  for at most the given number of cells

.. note:: *rate* and *replenish_interval* is conflict with each other, the latter one in conf will take effect.

The yaml value for *u32 limit quota* can be in varies formats:

* simple rate

  Just the rate value. The max_burst value is the same as the one set in the rate.

* map

  The keys of this map are the fields as described above.

.. _conf_value_random_ratio:

random ratio
============

**yaml value**: f64 | str | bool | integer

Set a random ratio between 0.0 and 1.0 (inclusive).

For *str* value, it can be in fraction form (n/d), in percentage form (n%), or just a float string.

For *bool* value, *false* means 0.0, *true* means 1.0.

For *integer* value, only 0 and 1 is allowed.
