.. _configuration_user_group_source_file:

File
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
