#!/bin/sh

set -e

SCRIPTS_DIR=$(dirname "$0")
PROJECT_DIR=$(realpath "${SCRIPTS_DIR}/..")

cd "${PROJECT_DIR}"

CODENAME=$(lsb_release -c -s)

echo "Finalize debian/changelog"
dch --maintmaint --release --distribution "${CODENAME}" --force-distribution "Finalize changelog"

export RUSTFLAGS="--remap-path-prefix ${HOME}=~"

dpkg-buildpackage -b -uc
