.. _protocol_helper:

###############
Helper Protocol
###############

route_query
===========

This protocol is used to make queries in route_query escaper. See :doc:`route_query`.

cert_generator
==============

This protocol is used by auditor when do TLS interception. See :doc:`cert_generator`.

ip_locate
=========

This protocol is used by route_geoip escaper to find IP locations. See :doc:`ip_locate`.

icap_h2
=======

This tells what's needed to enable ICAP for HTTP/2.0.

icap_imap
=========

This tells what's needed to enable ICAP for IMAP.

icap_smtp
=========

This tells what's needed to enable ICAP for SMTP.

stream_detour
=============

The protocol is used in auditor to send client/remote streams to external interception server.

.. toctree::
   :hidden:

   route_query
   cert_generator
   ip_locate
   icap_http
   icap_h2
   icap_imap
   icap_smtp
   stream_detour
