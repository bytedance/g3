#!/bin/sh

set -e

if $(pkg-config --atleast-version 1.1.1 libssl)
then
	:
else
	echo "vendored-openssl"
fi

