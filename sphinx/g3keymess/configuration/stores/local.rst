.. _configuration_store_local:

local
=====

This is the local store that just load private keys in local directory.

The following keys are supported:

directory
---------

**required**, **type**: :ref:`directory path <conf_value_directory_path>`

Set the path of the local directory that contained the private keys.

watch
-----

**optional**, **type**: bool

Enable write watch of the .key files under the store directory.

The new written keys will be loaded automatically after we receive a write-done event.

This is only supported on Linux.

**default**: false
