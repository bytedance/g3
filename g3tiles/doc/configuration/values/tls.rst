
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

.. _conf_value_tls_version:

tls version
===========

**yaml type**: string / f64

Set TLS version to use.

The valid string values are: tls1.0, tls1.1, tls1.2, tls1.3.
The valid f64 values are: 1.0, 1.1, 1.2, 1.3.

.. _conf_value_tls_ticketer:

tls ticketer
============

**yaml type**: map

A rolling TLS ticketer which support:

- new encrypt key generation
- old decrypt key update / expire check
- sync keys from remote source

The supported fields are:

* check_interval

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the check interval for key expiration. It will also try to fetch keys from remote source.

  If the encryption key is expired with it's lifetime, and no new key is fetched from remote source,
  a new random key will be generated locally.

  **default**: 5min

* local_lifetime

  **optional**, **type**: u32

  Set the lifetime value (in seconds) for local generated key. The expire time will be half of this lifetime value.

  **default**: 12 * 3600

* source

  **optional**, **type**: :ref:`tls ticket remote source <conf_value_tls_ticket_remote_source>`

  Set the remote source to use.

  **default**: not set

.. _conf_value_tls_ticket_remote_source:

tls ticket remote source
========================

**yaml type**: map

Set the remote source to fetch TLS ticket keys.

There may be many types of sources available, the **type** config key should be used to set the remote source type.

Key Format
----------

.. _conf_value_tls_ticket_encrypt_key:

encrypt key
^^^^^^^^^^^

**json type**: map

The config keys are:

* name

  **required**, **type**: hex str

  Set the name, which should be of 16 bytes.

* aes

  **required**, **type**: hex str

  Set the AES KEY, which should be of 32 bytes.

* hmac

  **required**, **type**: hex str

  Set the HMAC KEY, which should be of 16 bytes.

* lifetime

  **optional**, **type**: u32

  Set the lifetime value.

  **default**: 24 * 3600

.. _conf_value_tls_ticket_decrypt_key:

decrypt key
^^^^^^^^^^^

**json type**: map

The config keys are:

* name

  **required**, **type**: hex str

  Set the name, which should be of 16 bytes.

* aes

  **required**, **type**: hex str

  Set the AES KEY, which should be of 32 bytes.

* hmac

  **required**, **type**: hex str

  Set the HMAC KEY, which should be of 16 bytes.

* expire

  **required**, **type**: :ref:`rfc3339 datetime str <conf_value_rfc3339_datetime_str>`

  Set the expire datetime.

Source Types
------------

redis
^^^^^

**yaml type**: map

A redis TLS ticket key source.

The following keys are supported:

* enc_key

  **required**, **type**: str

  Set the redis key name that will contain the :ref:`encrypt key <conf_value_tls_ticket_encrypt_key>` json string.

* dec_set

  **required**, **type**: str

  Set the redis set name that will contain the :ref:`encrypt key <conf_value_tls_ticket_decrypt_key>` json strings.

* :ref:`nested redis config map <conf_value_db_redis>`

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

.. _conf_value_openssl_protocol:

openssl protocol
================

**yaml value**: string

Set openssl protocol version.

Current supported values are:

- tls1.0
- tls1.1
- tls1.2
- tls1.3
- tlcp (only if vendored-tongsuo feature is enabled)

.. _conf_value_openssl_ciphers:

openssl ciphers
===============

**yaml value**: string or seq

Set openssl cipher list or ciphersuites for the specified protocol.

Values can be obtained from `openssl ciphers -v` command.

For string value, it can be ciphers joined by ':'.

For seq value, each one should be a cipher string.

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

* min_tls_version

  **optional**, **type**: :ref:`tls version <conf_value_tls_version>`

  Set the minimal TLS version to use if `protocol` is not set.

  **default**: not set

  .. versionadded:: 0.3.5

* max_tls_version

  **optional**, **type**: :ref:`tls version <conf_value_tls_version>`

  Set the maximum TLS version to use if `protocol` is not set.

  **default**: not set

  .. versionadded:: 0.3.5

* ciphers

  **optional**, **type**: :ref:`openssl ciphers <conf_value_openssl_ciphers>`
  **require**: protocol

  Set to use a specific set of ciphers for the specified protocol version.

  **default**: not set

* disable_sni

  **optional**, **type**: bool

  Whether to send the Server Name Indication (SNI) extension during the client handshake.

  **default**: false

* cert_pair

  **optional**, **type**: :ref:`tls cert pair <conf_value_tls_cert_pair>`
  **conflict**: certificate, private_key

  Set the client certificate and private key pair.

  **default**: not set

* tlcp_cert_pair

  **optional**, **type**: :ref:`tlcp cert pair <conf_value_tlcp_cert_pair>`

  Set the client certificate and private key pair for TLCP protocol.
  This will be in effect only if protocol is set to tlcp.

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

* session_cache_lru_max_sites

  **optional**, **type**: usize

  Set how many LRU sites should have cached sessions.

  Only in use if this tls client is used by many sites.

  **default**: 128

* session_cache_each_capacity

  **optional**, **type**: usize

  Set how many sessions should be kept for each site.

  **default**: 16

* supported_groups

  **optional**, **type**: str

  Set the supported elliptic curve groups.

  **default**: not set

* use_ocsp_stapling

  **optional**, **type**: bool

  Set this to true to request a stapled OCSP response from the server.

  Verify of this response is still not implemented.

  **default**: not set, the default value may vary between different OpenSSL variants

* enable_sct

  **optional**, **type**: bool

  Enable the processing of signed certificate timestamps (SCTs) for OpenSSL, or enables SCT requests for BoringSSL.

  Verify of this response is still not implemented for BoringSSL variants.

  **default**: not set, the default value may vary between different OpenSSL variants

* enable_grease

  **optional**, **type**: bool

  Enable GREASE. See `RFC 8701`_.

  **default**: not set, the default value may vary between different OpenSSL variants

  .. _RFC 8701: https://datatracker.ietf.org/doc/rfc8701/

* permute_extensions

  **optional**, **type**: bool

  Whether to permute TLS extensions.

  **default**: not set, the default value may vary between different OpenSSL variants

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

* disable_sni

  **optional**, **type**: bool

  Whether to send the Server Name Indication (SNI) extension during the client handshake.

  **default**: false

* max_fragment_size

  **optional**, **type**: usize

  Set the maximum size of TLS message we'll emit.

  **default**: default value in tls driver

* cert_pair

  **optional**, **type**: :ref:`tls cert pair <conf_value_tls_cert_pair>`
  **conflict**: certificate, private_key

  Set the client certificate and private key pair.

  **default**: not set

* ca_certificate | server_auth_certificate

  **optional**, **type**: :ref:`tls certificates <conf_value_tls_certificates>`

  A list of certificates for server auth. If not set, the system default ca certificates will be used.

  **default**: not set

* no_default_ca_certificate

  **optional**, **type**: bool

  Set if you don't want to load default ca certificates.

  **default**: false

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

* enable_client_auth

  **optional**, **type**: bool

  Set if you want to enable client auth.

  **default**: disabled

* no_session_ticket

  **optional**, **type**: bool

  Set if we should disable TLS session ticket (stateless session resumption by Session Ticket).

  **default**: false

  .. versionadded:: 0.3.3

* no_session_cache

  **optional**, **type**: bool

  Set if we should disable TLS session cache (stateful session resumption by Session ID).

  **default**: false

  .. versionadded:: 0.3.3

* ca_certificate | client_auth_certificate

  **optional**, **type**: :ref:`tls certificates <conf_value_tls_certificates>`

  A list of certificates for client auth. If not set, the system default ca certificates will be used.

  **default**: not set

* handshake_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the tls handshake timeout value.

  **default**: 10s
