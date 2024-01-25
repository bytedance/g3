
.. _configure_tls_value_types:

***
TLS
***

.. _conf_value_tls_name:

tls name
========

**yaml type**: :ref:`host <conf_value_host>`

Set the dns name / ip address for server certificate verification.
If not set, the corresponding upstream address will be used.

.. versionchanged:: IP address is supported since version 1.7.15

.. _conf_value_tls_certificates:

tls certificates
================

**yaml type**: :ref:`file <conf_value_file>` | seq

Set the certificate file(s), which should be in PEM format(`openssl-req(1)`_).

If relative, it will be searched in the directory that contains current config file.

.. _openssl-req(1): https://www.openssl.org/docs/manmaster/man1/openssl-req.html

.. _conf_value_tls_private_key:

tls private_key
===============

**yaml type**: :ref:`file <conf_value_file>`

Set the private key file, which should be in PKCS#8(`openssl-genpkey(1)`_) or traditional PEM format.

If relative, it will be searched in the directory that contains current config file.
The last one in the file will be used if many keys are found.

.. versionchanged:: support traditional PEM format since version 1.3.2

.. _openssl-genpkey(1): https://www.openssl.org/docs/manmaster/man1/openssl-genpkey.html

.. _conf_value_tls_cert_pair:

tls cert pair
=============

**yaml value**: map

A pair value contains tls certificate and private key.

The keys are:

* certificate

  **required**, **type**: :ref:`tls certificates <conf_value_tls_certificates>`

  Set client certificates if client auth is needed by remote server.
  Private key must also be set if client auth is needed.

  **default**: not set

* private_key

  **required**, **type**: :ref:`tls private_key <conf_value_tls_private_key>`

  Set the private key for client if client auth is needed by remote server.
  Client certificates are also needed if client auth is needed.

  **default**: not set

.. versionadded:: 1.7.7

.. _conf_value_tlcp_cert_pair:

tlcp cert pair
==============

**yaml value**: map

A pair value contains tlcp certificate and private key.

The keys are:

* sign_certificate

  **required**, **type**: :ref:`tls certificates <conf_value_tls_certificates>`

  Set client sign certificates if client auth is needed by remote server.
  Private key must also be set if client auth is needed.

  **default**: not set

* sign_private_key

  **required**, **type**: :ref:`tls private_key <conf_value_tls_private_key>`

  Set the sign private key for client if client auth is needed by remote server.
  Client certificates are also needed if client auth is needed.

  **default**: not set

* enc_certificate

  **required**, **type**: :ref:`tls certificates <conf_value_tls_certificates>`

  Set client enc certificates if client auth is needed by remote server.
  Private key must also be set if client auth is needed.

  **default**: not set

* enc_private_key

  **required**, **type**: :ref:`tls private_key <conf_value_tls_private_key>`

  Set the enc private key for client if client auth is needed by remote server.
  Client certificates are also needed if client auth is needed.

  **default**: not set

.. versionadded:: 1.7.23

.. _conf_value_openssl_protocol:

openssl protocol
================

**yaml value**: string

Set openssl protocol version.

Current supported values are:

- tls1.2
- tls1.3
- tlcp (only if vendored-tongsuo feature is enabled)

.. versionadded:: 1.7.7

.. _conf_value_openssl_ciphers:

openssl ciphers
===============

**yaml value**: string or seq

Set openssl cipher list or ciphersuites for the specified protocol.

Values can be obtained from `openssl ciphers -v` command.

For string value, it can be ciphers joined by ':'.

For seq value, each one should be a cipher string.

.. versionadded:: 1.7.7

.. _conf_value_openssl_tls_client_config:

openssl tls client config
=========================

**yaml value**: map

The tls config to be used as a tls client.

The map is consists of the following fields:

* protocol

  **optional**, **type**: :ref:`openssl protocol <conf_value_openssl_protocol>`

  Set to use a specific protocol version.

  **default**: not set

  .. versionadded:: 1.7.7

* ciphers

  **optional**, **type**: :ref:`openssl ciphers <conf_value_openssl_ciphers>`
  **require**: protocol

  Set to use a specific set of ciphers for the specified protocol version.

  **default**: not set

  .. versionadded:: 1.7.7

* disable_sni

  **optional**, **type**: bool

  Whether to send the Server Name Indication (SNI) extension during the client handshake.

  **default**: false

* cert_pair

  **optional**, **type**: :ref:`tls cert pair <conf_value_tls_cert_pair>`
  **conflict**: certificate, private_key

  Set the client certificate and private key pair.

  **default**: not set

  .. versionadded:: 1.7.7

* tlcp_cert_pair

  **optional**, **type**: :ref:`tlcp cert pair <conf_value_tlcp_cert_pair>`

  Set the client certificate and private key pair for TLCP protocol.
  This will be in effect only if protocol is set to tlcp.

  **default**: not set

  .. versionadded:: 1.7.23

* certificate

  **optional**, **type**: :ref:`tls certificates <conf_value_tls_certificates>`
  **conflict**: cert_pair

  Set client certificates if client auth is needed by remote server.
  Private key must also be set if client auth is needed.

  **default**: not set

* private_key

  **optional**, **type**: :ref:`tls private_key <conf_value_tls_private_key>`
  **conflict**: cert_pair

  Set the private key for client if client auth is needed by remote server.
  Client certificates are also needed if client auth is needed.

  **default**: not set

* ca_certificate | server_auth_certificate

  **optional**, **type**: :ref:`tls certificates <conf_value_tls_certificates>`

  A list of certificates for server auth. If not set, the system default ca certificates will be used.

  **default**: not set

* no_default_ca_certificate

  **optional**, **type**: bool

  Set if you don't want to load default ca certificates.

  **default**: false

* handshake_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the tls handshake timeout value.

  **default**: 10s

* no_session_cache

  **optional**, **type**: bool

  Set if you want to disable cache of TLS sessions.

  **default**: false

* use_builtin_session_cache

  **optional**, **type**: bool

  Set if we should use OpenSSL builtin session cache.

  **default**: false

  .. versionadded:: 1.7.7

* session_cache_lru_max_sites

  **optional**, **type**: usize

  Set how many LRU sites should have cached sessions.

  Only in use if this tls client is used by many sites.

  **default**: 128

* session_cache_each_capacity

  **optional**, **type**: usize

  Set how many sessions should be kept for each site.

  **default**: 16

.. versionadded:: 1.1.4

.. _conf_value_openssl_server_config:

openssl server config
=====================

**yaml value**: map

The tls config to be used as a openssl tls server.

The map is consists of the following fields:

* cert_pairs

  **optional**, **type**: :ref:`tls cert pair <conf_value_tls_cert_pair>` or seq

  Set certificate and private key pairs for this TLS server.

  If not set, TLS protocol will be disabled.

  **default**: not set

* tlcp_cert_pairs

  **optional**, **type**: :ref:`tlcp cert pair <conf_value_tlcp_cert_pair>` or seq

  Set certificate and private key pairs for this TLCP server.

  If not set, TLCP protocol will be disabled.

  **default**: not set

* enable_client_auth

  **optional**, **type**: bool

  Set if you want to enable client auth.

  **default**: disabled

* session_id_context

  **optional**, **type**: str

  A string that will be added to the prefix when calculate the session id context sha1 hash.

  .. versionadded:: 1.7.32

* ca_certificate | client_auth_certificate

  **optional**, **type**: :ref:`tls certificates <conf_value_tls_certificates>`

  A list of certificates for client auth. If not set, the system default ca certificates will be used.

  **default**: not set

* handshake_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the tls handshake timeout value.

  **default**: 10s

.. versionadded:: 1.7.29

.. _conf_value_rustls_client_config:

rustls client config
====================

**yaml value**: map

The tls config to be used as a tls client.

The map is consists of the following fields:

* no_session_cache

  **optional**, **type**: bool

  Set if you want to disable cache of TLS sessions.

  **default**: false

  .. versionadded:: 1.1.4

* disable_sni

  **optional**, **type**: bool

  Whether to send the Server Name Indication (SNI) extension during the client handshake.

  **default**: false

  .. versionadded:: 1.1.4

* max_fragment_size

  **optional**, **type**: usize

  Set the maximum size of TLS message we'll emit.

  **default**: default value in tls driver

* cert_pair

  **optional**, **type**: :ref:`tls cert pair <conf_value_tls_cert_pair>`
  **conflict**: certificate, private_key

  Set the client certificate and private key pair.

  **default**: not set

  .. versionadded:: 1.7.8

* certificate

  **optional**, **type**: :ref:`tls certificates <conf_value_tls_certificates>`

  Set client certificates if client auth is needed by remote server.
  Private key must also be set if client auth is needed.

  **default**: not set

* private_key

  **optional**, **type**: :ref:`tls private_key <conf_value_tls_private_key>`

  Set the private key for client if client auth is needed by remote server.
  Client certificates are also needed if client auth is needed.

  **default**: not set

* ca_certificate | server_auth_certificate

  **optional**, **type**: :ref:`tls certificates <conf_value_tls_certificates>`

  A list of certificates for server auth. If not set, the system default ca certificates will be used.

  **default**: not set

* no_default_ca_certificate

  **optional**, **type**: bool

  Set if you don't want to load default ca certificates.

  **default**: false

  .. versionadded:: 1.1.4

* use_builtin_ca_certificate

  **optional**, **type**: bool

  Set to true if you want to use built in webpki-roots ca certificates as default ca certificates.

  **default**: false

* handshake_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the tls handshake timeout value.

  **default**: 10s

.. _conf_value_rustls_server_config:

rustls server config
====================

**yaml value**: map

The tls config to be used as a tls server.

The map is consists of the following fields:

* cert_pairs

  **optional**, **type**: :ref:`tls cert pair <conf_value_tls_cert_pair>` or seq

  Set certificate and private key pairs for this TLS server.

  .. note:: At least set this or certificate & private_key.

  .. versionadded:: 1.7.8

* certificate

  **optional**, **type**: :ref:`tls certificates <conf_value_tls_certificates>`

  Set the certificates for this TLS server.

  .. note:: At least set this or cert_pairs

* private_key

  **optional**, **type**: :ref:`tls private_key <conf_value_tls_private_key>`

  Set the private key for this TLS server.

  .. note:: At least set this or cert_pairs

* enable_client_auth

  **optional**, **type**: bool

  Set if you want to enable client auth.

  **default**: disabled

* use_session_ticket

  **optional**, **type**: bool

  Set if we should enable TLS session ticket to do TLS Session Resumption without Server-Side State.

  .. versionchanged:: 1.7.28

  **default**: disabled

* ca_certificate | client_auth_certificate

  **optional**, **type**: :ref:`tls certificates <conf_value_tls_certificates>`

  A list of certificates for client auth. If not set, the system default ca certificates will be used.

  **default**: not set

* handshake_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the tls handshake timeout value.

  **default**: 10s
