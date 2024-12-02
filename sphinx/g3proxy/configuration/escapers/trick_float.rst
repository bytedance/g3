.. _configuration_escaper_trick_float:

trick_float
===========

This escaper allows to select a next float escaper weighted randomly.

No common keys are supported.

next
----

**required**, **type**: seq

This set all the next escapers. Each element should be the name of the target float escaper.

.. note:: Duplication of next escapers will be ignored.
