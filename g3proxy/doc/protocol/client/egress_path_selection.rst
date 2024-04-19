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

The supported method is :ref:`by index <proto_egress_path_selection_by_index>`.

See :ref:`path_selection_header <config_server_http_proxy_egress_path_selection_header>` for more info.

socks extension
---------------

Only socks proxy server can support this.

No implementation for now.

username extension
------------------

All servers which support user auth with a username can support this.

No implementation for now.

user support
============

User level egress path selection can be enabled via :ref:`egress_path <config_user_egress_path>` config option.

The supported method is :ref:`by map <proto_egress_path_selection_by_map>`.

selection methods
=================

.. _proto_egress_path_selection_by_index:

by index
--------

**value**: usize

For escapers with multiple nodes (may be next escapers or ip addresses), the node with the specified index will be used.

The value will be wrapped into range *1 - len(nodes)*.
**NOTE*** the start value is *1*, *0* is the same as *len(nodes) - 1*.

.. _proto_egress_path_selection_by_map:

by map
------

**value**: json object

The root value should be a json map.

The key should be the escaper name, so the corresponding value will be handled by that escaper.

The value should be a `ID` string value, and it's meaning will be different on each type of escaper.
