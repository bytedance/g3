#!/bin/sh

set -e

SCRIPTS_DIR=$(dirname "$0")
PROJECT_DIR=$(realpath "${SCRIPTS_DIR}/..")

cd "${PROJECT_DIR}"

export RUSTFLAGS="--remap-path-prefix ${HOME}=~"

rpmbuild -bb --build-in-place *.spec
