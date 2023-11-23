#!/bin/sh

set -e

if $(pkg-config --atleast-version 3.0.0 libssl)
then
	:
else
	echo "vendored-openssl"
fi

