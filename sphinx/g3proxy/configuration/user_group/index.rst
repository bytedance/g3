.. _configuration_user_group:

**********
User Group
**********

The type for each user group config is *map*, with two always required keys:

* name

  **type**: :ref:`metric node name <conf_value_metric_node_name>`

  The name of the user group.

* type

  **type**: str

  The real type of the user group, decides how to parse other keys.

For now, we only support *hashed_user* type of user group. We may add a *gss_api* type after sometime.

The real auth type used in each protocol is determined by the type of user group.
See documentation for each server type for the mapping.

Fast Link
=========

.. toctree::
   :maxdepth: 1

   user
   source
   audit
   site
   name_params

Group types
===========

hashed_user
-----------

This type of user group is consist of users that store hashed passwords. The clear text password must be transported
to server, so we can calc its hash and compare with the ones in config.

The users are split into two groups: static and dynamic. The static users is configured with key *static_users*
in config file, each in yaml format. The dynamic users are fetched from *dynamic_source* periodically, each in json
format. Both can be optional and share the same data structure.

The Cap'n Proto RPC publish_dynamic_users command is supported, the published data should be an array of
:ref:`user <configuration_user_group_user>`.

* static_users

  **optional**, **type**: seq

  Static user can be added in this array.

  See :ref:`user <configuration_user_group_user>` for detailed structure of user.

* source

  **optional**, **type**: :ref:`url str <conf_value_url_str>` | map

  Set the fetch source for dynamic users.

  We support many type of sources. The type is detected by reading the *scheme* field of url,
  or the *type* key of the map. See :ref:`source <configuration_user_group_source>` for all supported type of sources.

  Dynamic users will be updated if they exist already. If the fetch fail, we will continue keep the old users.

  **default**: not set

.. _conf_user_group_cache:

* cache

  **optional**, **type**: :ref:`file path <conf_value_file_path>`

  The local file to cache remote results, it will be used during initial load of the user group.

  The file will be created if not existed.

  This will overwrite the *cache_file* options in :ref:`source <configuration_user_group_source>` config.

  .. note:: This should be set if you want to publish dynamic users.

  **default**: not set

  .. versionadded:: 1.7.22

.. _conf_user_group_refresh_interval:

* refresh_interval

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the check interval for user expiration and the fetch interval for dynamic users.

  **default**: 60s

* anonymous_user

  **optional**, **type**: :ref:`user <configuration_user_group_user>`

  Set and enable the anonymous user.

  This will be used if no correct username could be found in both static and dynamic users,
  or no auth info is carried in the client request.

  **default**: not set

  .. versionadded:: 1.7.13
