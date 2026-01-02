.. _configuration_auth_user_group:

**********
User Group
**********

The users are split into two groups: static and dynamic.
The static users is configured with :ref:`static users <conf_auth_user_group_static_users>` in config file, each in yaml format.
The dynamic users are fetched from :ref:`source <conf_auth_user_group_source>` periodically, each in json format.
Both can be optional and share the same data structure.

The Cap'n Proto RPC publish_dynamic_users command is supported, the published data should be an array of
:ref:`user <configuration_auth_user>`.

The type for each user group config is *map*, with two always required keys:

* :ref:`name <conf_auth_user_group_name>` user group name
* :ref:`type <conf_auth_user_group_type>` authenticate type

Groups
======

.. toctree::
   :maxdepth: 1

   basic
   facts

Common Keys
===========

.. _conf_auth_user_group_name:

name
----

**required**,  **type**: :ref:`metric node name <conf_value_metric_node_name>`

The name of the user group.

.. _conf_auth_user_group_type:

type
----

**required**, **type**: str

The authenticate type of the user group, also decides how to parse other keys.

.. _conf_auth_user_group_static_users:

**default**: basic

.. versionadded:: 1.13.0

static_users
------------

**optional**, **type**: seq

Static user can be added in this array.

See :ref:`user <configuration_auth_user>` for detailed structure of user.

.. _conf_auth_user_group_source:

source
------

**optional**, **type**: :ref:`url str <conf_value_url_str>` | map

Set the fetch source for dynamic users.

We support many type of sources. The type is detected by reading the *scheme* field of url,
or the *type* key of the map. See :ref:`source <configuration_auth_user_source>` for all supported type of sources.

Dynamic users will be updated if they exist already. If the fetch fail, we will continue keep the old users.

**default**: not set

.. _conf_auth_user_group_cache:

cache
-----

**optional**, **type**: :ref:`file path <conf_value_file_path>`

The local file to cache remote results, it will be used during initial load of the user group.

The file will be created if not existed.

.. note:: This should be set if you want to publish dynamic users.

**default**: not set

.. versionadded:: 1.7.22

.. _conf_auth_user_group_refresh_interval:

refresh_interval
----------------

**optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

Set the check interval for user expiration and the fetch interval for dynamic users.

**default**: 60s

.. _conf_auth_user_group_anonymous_user:

anonymous_user
--------------

**optional**, **type**: :ref:`user <configuration_auth_user>`

Set and enable the anonymous user.

This will be used if no correct username could be found in both static and dynamic users,
or no auth info is carried in the client request.

**default**: not set

.. versionadded:: 1.7.13
