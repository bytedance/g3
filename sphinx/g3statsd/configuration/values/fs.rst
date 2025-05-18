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
