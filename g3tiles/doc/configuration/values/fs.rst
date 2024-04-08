
.. _configure_fs_value_types:

**********
Filesystem
**********

.. _conf_value_hybrid_map:

hybrid map
==========

**yaml value**: seq | str

This is a hybrid container for a sequence of maps which may reside in other files.

For *seq* value, all of it's values should be the final map, or a valid *str* value as described below.

For *str* value, it should be a valid path, which may be absolute or relative to the directory of the main conf file.

The path may be a file or directory:

* If the path is a directory, the non-symbolic files in it with extension *.conf* will be parsed as described below.
* If the path is a file, it should contains one or many yaml docs, each doc will be the final map.

.. _conf_value_file_path:

file path
=========

**yaml value**: str

This set the path for a regular file to be used.

The file should be an absolute path, or relative to a predefined path.

The path should be existed, or can be auto created, according to the specific config.

.. _conf_value_file:

file
====

**yaml value**: str

This set a file to be read. The file should be an absolute path, or relative to a predefined path.

.. _conf_value_absolute_path:

absolute path
=============

**yaml value**: str

The set a file path to be used. The path should be absolute.

.. _conf_value_config_file_format:

config file format
==================

**yaml value**: str

Set the format for the related config file.

The following values are supported:

* yaml
* json

The default vary in different contexts.
