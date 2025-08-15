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
	echo "Building temp rpm package for ${PACKAGE}"
fi

cd "${PROJECT_DIR}"

echo "Generate license files for bundled crates"
cargo metadata --format-version 1 | scripts/release/bundle_license.py > LICENSE-BUNDLED

if [ -f ${PACKAGE}/doc/conf.py ]
then
	echo "Building sphinx docs"
	sphinx-build -q -b html ${PACKAGE}/doc ${PACKAGE}/doc/_build/html
fi

SPEC_FILE="${PACKAGE}.spec"
[ ! -e "${SPEC_FILE}" ] || rm "${SPEC_FILE}"
cp "${PACKAGE}/${PACKAGE}.spec" "${SPEC_FILE}"

SRC_VERSION=$(cargo read-manifest --offline --manifest-path "${PACKAGE}"/Cargo.toml | jq -r '.version')
VERSION=$(rpmspec -q --srpm --qf "%{version}" "${SPEC_FILE}")

set_dev_version()
{
	GIT_VER=$(git log -1 --pretty=format:%cdgit%h --date=format:%Y%m%d)
	echo "Git version: ${GIT_VER}"

	echo "Finalize ${SPEC_FILE}"
	# see https://docs.fedoraproject.org/en-US/packaging-guidelines/Versioning/
	NEW_VERSION="${VERSION}${VERSION_SYMBOL}${GIT_VER}"
	rpmdev-bumpspec -n "${NEW_VERSION}" -c "new git snapshot build" -u "G3proxy Maintainers <g3proxy-maintainers@devel.machine>" "${SPEC_FILE}"
}

echo "Looking for previous release tag"
TAG_FORMAL="${PACKAGE}-v${SRC_VERSION}"
TAG_REF_NAME=$(git describe --match "${TAG_FORMAL}" || :)
VERSION_SYMBOL=""
if [ -n "${TAG_REF_NAME}" ]
then
	if [ "${TAG_REF_NAME}" != "${TAG_FORMAL}" ]
	then
		VERSION_SYMBOL="^"
		echo "This is an update for formal version ${VERSION}"

		set_dev_version
	else
		echo "Build for formal version ${TAG_FORMAL}"
	fi
else
	VERSION_SYMBOL="~"
	echo "This is an pre-release for version ${VERSION}"

	set_dev_version
fi

export RUSTFLAGS="--remap-path-prefix ${HOME}=~"

rpmbuild -bb --build-in-place "${SPEC_FILE}"
