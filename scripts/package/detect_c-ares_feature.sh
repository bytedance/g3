#!/bin/sh

set -e

if $(pkg-config --atleast-version 1.18.0 libcares)
then
	echo "c-ares"
else
	echo "vendored-c-ares"
fi
