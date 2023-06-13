.. _metrics:

#######
Metrics
#######

Currently we only support send metrics to statsd, see :ref:`stat <configuration_stat>` for more details.

The following is the common tags for all metrics:

.. _metrics_tag_daemon_group:

* daemon_group

  This tag is the same as the daemon group specified in config file or command args.

.. _metrics_tag_stat_id:

* stat_id

  A machine local unique stat_id for dedup purpose. It should be **dropped** by statsd, and the metrics with the same
  remaining tags should be aggregated.

.. _metrics_tag_transport:

* transport

  Show the transport layer protocol. Values are:

  - tcp
  - udp

.. _metrics_tag_connection:

* connection

  Show the client connection type. Values are:

  - http
  - socks

.. _metrics_tag_request:

* request

  Show the request type. Values ars:

  - http_forward
  - https_forward
  - http_connect
  - socks_tcp_connect
  - socks_udp_connect
  - socks_udp_associate

.. toctree::

   server
   escaper
   resolver
   user
   user_site
   logger
