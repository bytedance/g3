.. _configuration_user_group:

**********
User Group
**********

.. toctree::
   :maxdepth: 2

   user
   source
   audit
   site

The type for each user group config is *map*, with two always required keys:

* *name*, which specify the name of the user group.
* *type*, which specify the real type of the user group, decides how to parse other keys.

For now, we only support *hashed_user* type of user group. We may add a *gss_api* type after sometime.

The real auth type used in each protocol is determined by the type of user group.
See documentation for each server type for the mapping.

Group types
===========

hashed_user
-----------

This type of user group is consist of users that store hashed passwords. The clear text password must be transported
to server, so we can calc its hash and compare with the ones in config.

The users are split into two groups: static and dynamic. The static users is configured with key *static_users*
in config file, each in yaml format. The dynamic users are fetched from *dynamic_source* periodically, each in json
format. Both can be optional and share the same data structure.

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

.. _conf_user_group_refresh_interval:

* refresh_interval

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the check interval for user expiration and the fetch interval for dynamic users.

  **default**: 60s
