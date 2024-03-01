.. _protocol_setup_wireshark_udpdump:

=================
Wireshark Udpdump
=================

See `udpdump(1)`_ for more introduction.

.. _udpdump(1): https://www.wireshark.org/docs/man-pages/udpdump.html

Protocol
--------

Data structure: `exported_pdu_tlvs`_

Dissector Code: `exported_pdu`_

.. _exported_pdu_tlvs: https://github.com/wireshark/wireshark/blob/master/wsutil/exported_pdu_tlvs.h
.. _exported_pdu: https://github.com/wireshark/wireshark/blob/master/epan/dissectors/packet-exported_pdu.c


Wireshark GUI
-------------

Steps to capture:

- Select *UDP Listener remote capture* line in wireshark main GUI.
- Click *setting* button at the beginning of that line.
- Set payload type to **exported_pdu**.
- Set listen port to whatever you want, and click *save*.
- Double click the *UDP Listener remote capture* line to start the capture.

Tshark CLI
----------

Doc: `extcap-preference`_.

.. _extcap-preference: https://tshark.dev/capture/sources/extcap_interfaces/#extcap-preferences

Example:

.. code-block:: shell

  tshark -i udpdump -o extcap.udpdump.payload:exported_pdu -o extcap.udpdump.port:5555 <...>


