.. _configuration_auth_user_group_basic:

Basic
=====

This type of user group is consist of users that have hashed passwords in :ref:`token <conf_auth_user_token>` config field.

User will be selected by it's username, and the clear text password must be transported to server, then hashed and compared.

The following keys are supported:

* :ref:`name <conf_auth_user_group_name>`
* :ref:`type <conf_auth_user_group_type>`
* :ref:`static users <conf_auth_user_group_static_users>`
* :ref:`source <conf_auth_user_group_source>`
* :ref:`cache <conf_auth_user_group_cache>`
* :ref:`refresh_interval <conf_auth_user_group_refresh_interval>`
* :ref:`anonymous_user <conf_auth_user_group_anonymous_user>`
