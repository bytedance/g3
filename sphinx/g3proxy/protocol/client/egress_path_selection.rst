.. _protocol_egress_path_selection:

#####################
Egress Path Selection
#####################

Usually there are many outgoing ip addresses on proxy machine, and we may provide one to one server port mapping to
each of them.

In most cases, we may have one server port mapped to many outgoing ip addresses, and by default using a random selection
policy. But sometimes, users may want to specify which outgoing IP address to use.
Instead of setting up a lot of servers and escapers that are mapped together, we can use only a single pair of server
and escaper with the help of `egress path selection`.

For path selection to work, the escapers used must support and enable it.
Not all escapers support it, see the config documentation for each escaper for confirmation.

server support
==============

custom http header
------------------

Only http proxy server can support this.

The supported method is :ref:`number id <proto_egress_path_selection_number_id>`.

See :ref:`path_selection_header <config_server_http_proxy_egress_path_selection_header>` for more info.

socks extension
---------------

Only socks proxy server can support this.

No implementation for now.

username extension
------------------

All servers which support user auth with a username can support this.

The supported method is :ref:`egress upstream <proto_egress_path_selection_egress_upstream>`.

See :ref:`username_params <config_auth_username_params>` for more info.

user support
============

User level egress path selection can be enabled via:

- :ref:`egress_path_id_map <config_user_egress_path_id_map>` for :ref:`string id <proto_egress_path_selection_string_id>` egress path selection

- :ref:`egress_path_value_map <config_user_egress_path_value_map>` for :ref:`json value <proto_egress_path_selection_json_value>` egress path selection

selection values
================

The egress path selection data structure contains many maps.

All of these maps have escaper name as their key, and each escaper will fetch it's corresponded selection value.

The value types are:

.. _proto_egress_path_selection_number_id:

number id
---------

**value**: map

The value should be a usize value, which will be used as an index.

For escapers with multiple nodes (may be next escapers or ip addresses), the node with the specified index will be used.
The value will be wrapped into range *1 - len(nodes)*.
**NOTE*** the start value is *1*, *0* is the same as *len(nodes) - 1*.

.. _proto_egress_path_selection_string_id:

string id
---------

**value**: map

The value should be a `ID` string value, and it's meaning will be different on each type of escaper.

.. _proto_egress_path_selection_json_value:

json value
----------

**value**: map

The value should be a `JSON MAP` object (or a JSON MAP str in yaml config), and it's meaning will be different on each type of escaper.

.. _proto_egress_path_selection_egress_upstream:

egress upstream
---------------

**value**: map

The value should be a map with the following keys:

* addr

  **value**: :ref:`upstream str <conf_value_upstream_str>`

  It will override the upstream address used by the corresponding escaper.

* resolve_sticky_key

  **value**: string

  Resolve the upstream domain by using ketama consistent hash, and use this value as the hash key.
