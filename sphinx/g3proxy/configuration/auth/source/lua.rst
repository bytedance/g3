.. _configuration_user_group_source_lua:

Lua
===

Fetch users through local lua script.

The following vars will be defined when running the script:

* __file__

  This will be the absolute path of the script file

  .. versionadded:: 1.11.0

The return value of the script should be the json encoded string of all dynamic users.

.. note::

  Environment variable `LUA_PATH`_ and `LUA_CPATH`_ can be set to include more lua module files.
  Any ";;" in the value of the *LUA_PATH* environment variable will be replaced by the default path.

  .. _LUA_PATH: https://www.lua.org/manual/5.1/manual.html#pdf-package.path
  .. _LUA_CPATH: https://www.lua.org/manual/5.1/manual.html#pdf-package.cpath

The keys used in *map* format are:

* fetch_script

  **required**, **type**: :ref:`file path <conf_value_file_path>`

  The path of the lua script to fetch dynamic users.

  The content of this script file should be like:

  .. code-block:: lua

    -- TODO fetch users
    local result = "[]"
    -- return the json encoded string
    return result

  **alias**: script

* fetch_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout value for the execution of the fetch script.

  It's not recommended to set the timeout value greater the :ref:`refresh_interval <conf_auth_user_group_refresh_interval>`
  in group config.

  **default**: 30s, **alias**: timeout

* report_script

  **optional**, **type**: :ref:`file path <conf_value_file_path>`

  The path of the lua script to report the parse result of the fetched dynamic users.

  Two global functions should be defined in this script, like this:

  ..  code-block:: lua

    function reportOk ()
      -- takes no argument
    end

    function reportErr (errMsg)
      -- takes one argument, which the error message string
    end

* report_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout value for the execution of the report script.

  It's not recommended to set the timeout value greater the :ref:`refresh_interval <conf_auth_user_group_refresh_interval>`
  in group config.

  **default**: 15s, **alias**: timeout
