.. _configuration_user_group_source:

******
Source
******

For the *map* format, the **type** key should always be set.

file
====

Fetch dynamic users from a local file.

The content of the file should be the json encoded string of all dynamic users.

The keys used in *map* format are:

* path

  **required**, **type**: :ref:`file path <conf_value_file_path>`

  Set path for the file. The file should be existed before start the daemon.

* format

  **optional**, **type**: :ref:`config file format <conf_value_config_file_format>`

  Set the file format for the file specified in *path*.

  **default**: If the file has extension, the extension will be used to detect the format.
  If not format can be detected through extension, *yaml* will be used.

For *url* str values, the *path* should be an absolute path with the following format:

    file://<path>[?[format=<format>]]

.. note:: The published users won't be cached if you use static file source.

lua
===

.. versionadded:: 1.1.3

Fetch users through local lua script.

.. note::

  Environment variable `LUA_PATH`_ and `LUA_CPATH`_ can be set to include more lua module files.
  Any ";;" in the value of the *LUA_PATH* environment variable will be replaced by the default path.

  .. _LUA_PATH: https://www.lua.org/manual/5.1/manual.html#pdf-package.path
  .. _LUA_CPATH: https://www.lua.org/manual/5.1/manual.html#pdf-package.cpath


The return value of the script should be the json encoded string of all dynamic users.

The keys used in *map* format are:

* cache_file

  **required**, **type**: :ref:`file path <conf_value_file_path>`

  The local file to cache results, it will be used during initial load of the user group.

  The file will be created if not existed.

  This will be overwritten by the user-group level :ref:`cache <conf_user_group_cache>` config.

  .. deprecated:: 1.7.22 use user-group level cache config option

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

  It's not recommended to set the timeout value greater the :ref:`refresh_interval <conf_user_group_refresh_interval>`
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

  .. versionadded:: 1.4.1

* report_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout value for the execution of the report script.

  It's not recommended to set the timeout value greater the :ref:`refresh_interval <conf_user_group_refresh_interval>`
  in group config.

  **default**: 15s, **alias**: timeout

  .. versionadded:: 1.4.1

python
======

.. versionadded:: 1.5.0

Fetch users through local python script.

The keys used in *map* format are:

* cache_file

  **required**, **type**: :ref:`file path <conf_value_file_path>`

  The local file to cache results, it will be used during initial load of the user group.

  The file will be created if not existed.

  This will be overwritten by the user-group level :ref:`cache <conf_user_group_cache>` config.

  .. deprecated:: 1.7.22 use user-group level cache config option

* script

  **required**, **type**: :ref:`file path <conf_value_file_path>`

  The path of the python script.

  Three global functions should be defined in this script, like this:

  ..  code-block:: python

    def fetch_users():
        # required, takes no argument, returns the json string
        return "[]"

    def report_ok():
        # optional, takes no argument
        pass

    def report_err(errmsg):
        # optional, takes one positional argument, which is the error message string
        pass

* fetch_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout value for the execution of the fetch function.

  It's not recommended to set the timeout value greater the :ref:`refresh_interval <conf_user_group_refresh_interval>`
  in group config.

  **default**: 30s, **alias**: timeout

* report_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout value for the execution of the report functions.

  It's not recommended to set the timeout value greater the :ref:`refresh_interval <conf_user_group_refresh_interval>`
  in group config.

  **default**: 15s, **alias**: timeout
