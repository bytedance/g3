.. _protocol_helper_icap_imap:

=============
ICAP for IMAP
=============

g3proxy support to enable ICAP reqmod services for outgoing IMAP APPEND message.

The mail message will be converted to an HTTP/1.1 PUT request, and then send to ICAP server.
And the response from the ICAP server will be sent to the upstream.

The size of the returned mail message should not be changed.

The following headers will be added in the ICAP request header:

- X-Transformed-From

  The value will be **IMAP**.

The following headers will be set in the HTTP PUT request:

- Content-Type

  The value will be "message/rfc822" for SMTP DATA message.

- X-IMAP-Message-Size

  The value will be the exact size of the mail message in the IMAP APPEND command.
  The ICAP server can modify the mail message but should not change the mail message size.

The body of the HTTP PUT request will be the corresponding mail message data.

Limitations
-----------

The mail message size should not be changed by the ICAP server.
