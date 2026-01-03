.. _configure_auth_value_types:

****
Auth
****

All auth value types are described here.

.. _conf_value_username:

username
========

**yaml value**: str

The UTF-8 username to be used in different contexts.
Should be less than or equal to 255 bytes.

.. _conf_value_password:

password
========

**yaml value**: str

The UTF-8 password to be used in different contexts.
Should be less than or equal to 255 bytes.

.. _conf_value_facts_match_value:

facts_match_value
=================

**yaml value**: str | map

The type and the value that facts auth will match.
It should be either `<fact-type>:<fact-value>` string or a map with a single `<fact-type>: <fact-value>` field.

The fact-type should be one of:

- ip

  `<fact-value>` should be :ref:`ip addr str <conf_value_ip_addr_str>`.
  It will match if the auth fact is exactly that IP address.

- net

  `<fact-value>` should be :ref:`ip network str <conf_value_ip_network_str>`.
  It will match if the auth fact is an IP address contained in that CIDR range and it's the smallest one.

.. versionadded:: 1.13.0
