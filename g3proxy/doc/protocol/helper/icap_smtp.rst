.. _protocol_helper_icap_smtp:

=============
ICAP for SMTP
=============

g3proxy support to enable ICAP reqmod services for outgoing SMTP DATA message.

The SMTP message will be converted to an HTTP/1.1 PUT request, and then send to ICAP server.
And the response from the ICAP server will be convert back to a SMTP message in the same format.

The following headers will be added in the ICAP request header:

- X-Transformed-From

  The value will be **SMTP**.

The following headers will be set in the HTTP PUT request:

- Content-Type

  The value will be "message/rfc822" for SMTP DATA message.

The body of the HTTP PUT request will be the corresponding SMTP message data.

Not Implemented
---------------

- BDAT message.
- BURL message.

The not implemented extensions will be disabled by default in auditor's
`smtp interception <conf_auditor_smtp_interception>` config.
