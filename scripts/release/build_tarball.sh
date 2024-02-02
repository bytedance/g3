#!/bin/sh

set -e

SCRIPT_DIR=$(dirname $(realpath $0))
RELEASE_TAG=$1
GIT_REVISION=${2:-$RELEASE_TAG}
BUILD_DIR=
CARGO_VENDOR_DIR="vendor"
CARGO_CONFIG_DIR=".cargo"
CARGO_CONFIG_FILE="${CARGO_CONFIG_DIR}/config.toml"

clear_build_dir()
{
	[ -n "${BUILD_DIR}" ] || return
	if [ -d "${BUILD_DIR}" ]
	then
		echo "clear temp build dir ${BUILD_DIR}"
		rm -rf "${BUILD_DIR}"
	fi
}
trap clear_build_dir EXIT

if [ -z "${RELEASE_TAG}" ]
then
	echo "release tag is required"
	exit 1
fi

SOURCE_NAME=$(echo "${RELEASE_TAG}" | sed 's/\(.*[^-]\)-v[0-9].*/\1/')
SOURCE_VERSION=$(echo "${RELEASE_TAG}" | sed 's/.*[^-]-v\([0-9].*\)/\1/')
if [ "${SOURCE_NAME}" = "${SOURCE_VERSION}" ]
then
	# no -v<version> found
	SOURCE_VERSION=$(cargo metadata --format-version 1 | jq -r ".packages[]|select(.name == \"$SOURCE_NAME\")|.version")
	echo "source version is not supplied and we will use ${SOURCE_VERSION}"
fi
PKG_VERSION=$(echo "${SOURCE_VERSION}" | tr '-' '.')
SOURCE_TIMESTAMP=$(git show -s --pretty="format:%ct" "${GIT_REVISION}^{commit}")

BUILD_DIR=$(mktemp -d "/tmp/build.${SOURCE_NAME}-${SOURCE_VERSION}.XXX")
echo "==> Use temp build dir ${BUILD_DIR}"


echo "==> cleaning local cargo checkouts"
cargo cache --autoclean


echo "==> adding source code from git"
git archive --format=tar --prefix="${SOURCE_NAME}-${PKG_VERSION}/" "${GIT_REVISION}" | tar -C "${BUILD_DIR}" -xf -
git submodule foreach "
    echo \"--> adding source code for submodule \${name}\"
    DIR=${BUILD_DIR}/${SOURCE_NAME}-${PKG_VERSION}/\${name}
    git archive --format=tar HEAD | tar -C \"\${DIR}\" -xf -
"

cd "${BUILD_DIR}/${SOURCE_NAME}-${PKG_VERSION}"

echo "==> adding incorporating source for BoringSSL"
./scripts/generate/boringssl/incorporate.sh

echo "==> cleaning useless source files"
local_dep_crates=$("${SCRIPT_DIR}"/list_local_deps.py --lock-file Cargo.lock --component "${SOURCE_NAME}")
local_lib_crates=
for dep in ${local_dep_crates}
do
	if [ -d "lib/${dep}" ]
	then
		local_lib_crates="${local_lib_crates} ${dep}"
	fi
done

useless_crates=$("${SCRIPT_DIR}"/prune_workspace.py --input Cargo.toml --output Cargo.toml --component ${SOURCE_NAME} ${local_lib_crates})
for _path in ${useless_crates}
do
	echo "    delete crate with path ${_path}"
	[ ! -d "${_path}" ] || rm -r "${_path}"
done


echo "==> cleaning useless cargo patches"
useless_patches=$(cargo tree 2>&1 >/dev/null | awk -f "${SCRIPT_DIR}"/useless_patch.awk)
[ -z "${useless_patches}" ] || "${SCRIPT_DIR}"/prune_patch.py --input Cargo.toml --output Cargo.toml ${useless_patches}


echo "==> adding vendor code from cargo"
[ -d "${CARGO_CONFIG_DIR}" ] || mkdir "${CARGO_CONFIG_DIR}"
if [ -f "${CARGO_CONFIG_FILE}" ]
then
	printf "\n# for vendor-source\n" >> "${CARGO_CONFIG_FILE}"
else
	: > "${CARGO_CONFIG_FILE}"
fi
mkdir "${CARGO_VENDOR_DIR}"
cargo vendor "${CARGO_VENDOR_DIR}" | tee -a "${CARGO_CONFIG_FILE}"


echo "==> generate license files for bundled crates"
cargo metadata --format-version 1 | "${SCRIPT_DIR}"/bundle_license.py > LICENSE-BUNDLED


if [ -f ${SOURCE_NAME}/doc/conf.py ]
then
	echo "==> building sphinx docs"
	sphinx-build -b html ${SOURCE_NAME}/doc ${SOURCE_NAME}/doc/_build/html
fi


echo "==> moving package files"
if [ -d ${SOURCE_NAME}/debian ]
then
	[ ! -d debian ] || rm -rf debian
	mv ${SOURCE_NAME}/debian .
fi

if [ -f ${SOURCE_NAME}/${SOURCE_NAME}.spec ]
then
	mv ${SOURCE_NAME}/${SOURCE_NAME}.spec ${SOURCE_NAME}.spec
fi


echo "==> building final tarball"
cd - >/dev/null
PERMISSION_OPTS="--mode=u=rwX,g=rwX,o=rX"
REPRODUCIBLE_OPTS="--mtime=@${SOURCE_TIMESTAMP} --owner=g3:1000 --group=g3:1000 --sort=name ${PERMISSION_OPTS}"
PROGRESS_OPTS="--checkpoint=100 --checkpoint-action=dot"
tar -Jcf "${SOURCE_NAME}-${PKG_VERSION}.tar.xz" ${REPRODUCIBLE_OPTS} ${PROGRESS_OPTS} -C "${BUILD_DIR}" .
echo

:
