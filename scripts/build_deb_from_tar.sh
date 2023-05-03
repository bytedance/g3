#!/bin/sh

set -e

SCRIPTS_DIR=$(dirname "$0")
PROJECT_DIR=$(realpath "${SCRIPTS_DIR}/..")

cd "${PROJECT_DIR}"

CODENAME=$(lsb_release -c -s)

echo "Finalize debian/changelog"
dch --maintmaint --release --distribution "${CODENAME}" --force-distribution "Finalize changelog"

SOURCE_NAME=$(dpkg-parsechangelog -SSource)
SOURCE_VERSION=$(dpkg-parsechangelog -SVersion | awk -F'-' '{print $1}')

echo "Repack ${SOURCE_NAME}_${SOURCE_VERSION}.orig.tar.xz"
PERMISSION_OPTS="--mode=u=rwX,g=rwX,o=rX"
REPRODUCIBLE_OPTS="--owner=g3:1000 --group=g3:1000 --sort=name ${PERMISSION_OPTS}"
PROGRESS_OPTS="--checkpoint=100 --checkpoint-action=dot"
cd ..
tar -Jcf "${SOURCE_NAME}_${SOURCE_VERSION}.orig.tar.xz" ${REPRODUCIBLE_OPTS} ${PROGRESS_OPTS} ${SOURCE_NAME}-${SOURCE_VERSION}
cd -
echo

echo "Building"
export RUSTFLAGS="--remap-path-prefix ${HOME}=~"

dpkg-buildpackage -uc -us
