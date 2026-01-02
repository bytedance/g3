.. _configuration_user_group_source_python:

Python
======

Fetch users through local python script.

The following vars will be defined when running the script:

* __file__

  This will be the absolute path of the script file

  .. versionadded:: 1.11.0

The keys used in *map* format are:

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

  It's not recommended to set the timeout value greater the :ref:`refresh_interval <conf_auth_user_group_refresh_interval>`
  in group config.

  **default**: 30s, **alias**: timeout

* report_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout value for the execution of the report functions.

  It's not recommended to set the timeout value greater the :ref:`refresh_interval <conf_auth_user_group_refresh_interval>`
  in group config.

  **default**: 15s, **alias**: timeout
