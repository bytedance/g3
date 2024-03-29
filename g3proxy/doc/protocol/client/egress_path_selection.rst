.. _protocol_egress_path_selection:

#####################
Egress Path Selection
#####################

Usually there are many outgoing ip addresses on proxy machine, and we may provide one to one server port mapping to
each of them. In many cases, we may have one server port mapping to many outgoing ip addresses, and we default to
use a random selection policy, which will match the most common use cases. But for some users, they may want to
connect to such one to many server port, but bind each connection to a specific ip address, so we need a path selection
policy which the user can tell us within each connection negotiation stage.

For path selection to work, the escapers used must support and enable it.
Not all escapers support it, see the config documentation for each one for confirmation.

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

The supported method is :ref:`by json <proto_egress_path_selection_by_json>`.

selection methods
=================

default
-------

**value**: constant("default")

The default one, just like no path selection enabled.

.. _proto_egress_path_selection_by_index:

by index
--------

**value**: usize

For escapers with multiple nodes (may be next escapers or ip addresses), the node with the specified index will be used.

The value will be wrapped into range *1 - len(nodes)*.
**NOTE*** the start value is *1*, *0* is the same as *len(nodes) - 1*.

.. _proto_egress_path_selection_by_json:

by json
-------

**value**: json object

The root value should be a json map.

The key should be the escaper name, so the corresponding value will be handled by that escaper.

The format of the value is defined by each :ref:`escaper <configuration_escaper>` that has such support.
