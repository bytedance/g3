.. _protocol_helper_icap_h2:

===========
ICAP for H2
===========

g3proxy support to enable ICAP reqmod and respmod services for H2 request and response.

The H2 request and response is convert to HTTP/1.1 first, and then send to ICAP server.
And the response from the ICAP server will be convert back to H2.

The following headers will be added in the ICAP request header:

- X-Transformed-From

  The value will be **HTTP/2.0**.

- X-HTTP-Upgrade

  The value will be the Protocol value set in Extended CONNECT request.
