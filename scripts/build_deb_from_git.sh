#!/bin/sh

set -e

SCRIPTS_DIR=$(dirname "$0")
PROJECT_DIR=$(realpath "${SCRIPTS_DIR}/..")

PACKAGE=$1
if [ -z "${PACKAGE}" ]
then
	echo "package name is required"
	exit 1
else
	echo "Building temp deb package for ${PACKAGE}"
fi

cd "${PROJECT_DIR}"

echo "Generate license files for bundled crates"
cargo metadata --format-version 1 | scripts/release/bundle_license.py > LICENSE-BUNDLED

if [ -f ${PACKAGE}/doc/conf.py ]
then
	echo "Building sphinx docs"
	sphinx-build -q -b html ${PACKAGE}/doc ${PACKAGE}/doc/_build/html
fi

[ ! -d debian ] || rm -rf debian
cp -r "${PACKAGE}/debian" .

SRC_VERSION=$(cargo read-manifest --offline --manifest-path "${PACKAGE}"/Cargo.toml | jq -r '.version')
VERSION=$(dpkg-parsechangelog -S Version | sed 's/\(.*\)-[^-]*/\1/')

echo "Looking for previous release tag"
TAG_REF_NAME=$(git describe --match "${PACKAGE}-v${SRC_VERSION}" || :)
VERSION_SYMBOL=""
if [ -n "${TAG_REF_NAME}" ]
then
	VERSION_SYMBOL="+"
	echo "This is an update for formal version ${VERSION}"
else
	VERSION_SYMBOL="~"
	echo "This is an pre-release for version ${VERSION}"
fi

GIT_VER=$(git log -1 --pretty=format:git%cd.%h --date=format:%Y%m%d)
echo "Git version: ${GIT_VER}"
NEW_VERSION="${VERSION}${VERSION_SYMBOL}${GIT_VER}-1"

CODENAME=$(lsb_release -c -s)
MAINTAINER=$(dpkg-parsechangelog -S Maintainer)
GIT_TS=$(git log -1 --pretty=format:%cd --date=format:%s)
DCH_TIME=$(LANG=en_US date -d @${GIT_TS} +"%a, %d %b %Y %H:%M:%S %z")

echo "Finalize debian/changelog"
cat << EOF > debian/changelog
${PACKAGE} (${NEW_VERSION}) ${CODENAME}; urgency=medium

  * New git snapshot.

 -- ${MAINTAINER}  ${DCH_TIME}
EOF

export RUSTFLAGS="--remap-path-prefix ${HOME}=~"

dpkg-buildpackage -b -uc
