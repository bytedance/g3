.. _log_escape_udp_sendto:

*********
UdpSendto
*********

The following keys are available for UdpSendto escape log:

next_expire
-----------

**optional**, **type**: rfc3339 timestamp string with microseconds

The expected expire time of the next peer.

Present only if the next escaper is dynamic and we have selected the remote peer.

reason
------

**required**, **type**: enum string

The brief error reason.
