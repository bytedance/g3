#!/bin/sh

set -e

if $(pkg-config --exists lua)
then
	pkg-config --variable=lib_name lua | tr -d '.'
else
	for lib in lua54 lua53 lua51
	do
		if $(pkg-config --exists ${lib})
		then
			echo "${lib}"
			break
		fi
	done
fi

