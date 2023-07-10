#!/bin/sh

set -e

CUR_DIR=$(dirname "${0}")

SYSTEMD_VERSION=$(pkg-config --modversion systemd | awk -F'.' '{print $1}')

if [ "${SYSTEMD_VERSION}" -lt "240" ]
then
	cp "${CUR_DIR}/g3keymess@.pre240.service" "${CUR_DIR}/g3keymess@.service"
else
	cp "${CUR_DIR}/g3keymess@.latest.service" "${CUR_DIR}/g3keymess@.service"
fi

