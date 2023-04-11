#!/bin/sh

set -e

SCRIPT_DIR=$(dirname $(realpath $0))
RELEASE_TAG=$1
BUILD_DIR=
CARGO_VENDOR_DIR="vendor"
CARGO_CONFIG_DIR=".cargo"
CARGO_CONFIG_FILE="${CARGO_CONFIG_DIR}/config"

lib_dependency()
{
	_path=$1

	cargo metadata --format-version 1 | \
		jq -r --arg path "${_path}" '.packages[]|select(.manifest_path|test($path))|.dependencies[].path|select(. != null and test("/lib/"))' | \
		sed 's/\(.*\/lib\/\)//' | sort | uniq
}

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
SOURCE_TIMESTAMP=$(git show -s --pretty="format:%ct" "${RELEASE_TAG}^{commit}")

BUILD_DIR=$(mktemp -d "/tmp/build.${SOURCE_NAME}-${SOURCE_VERSION}.XXX")
echo "==> Use temp build dir ${BUILD_DIR}"


echo "==> adding source code from git"
git archive --format=tar --prefix="${SOURCE_NAME}-${SOURCE_VERSION}/" "${RELEASE_TAG}" | tar -C "${BUILD_DIR}" -xf -

cd "${BUILD_DIR}/${SOURCE_NAME}-${SOURCE_VERSION}"


echo "==> cleaning useless source files"
SOURCE_PATH="$(pwd)/${SOURCE_NAME}"
lib_crates=$(lib_dependency "${SOURCE_PATH}")

next_check_crates="${lib_crates}"
while [ -n "${next_check_crates}" ]
do
	this_check_crates="${next_check_crates}"
	next_check_crates=""
	for _lib in ${this_check_crates}
	do
		nested_lib_crates=$(lib_dependency "${_lib}")
		for _nested_lib in ${nested_lib_crates}
		do
			_found=0
			for _e in ${lib_crates}
			do
				if [ "${_e}" = "${_nested_lib}" ]
				then
					_found=1
					break
				fi
			done
			if [ $_found -eq 0 ]
			then
				lib_crates="${lib_crates} ${_nested_lib}"
				next_check_crates="${next_check_crates} ${_nested_lib}"
			fi
		done
	done
done

useless_crates=$("${SCRIPT_DIR}"/prune_workspace.py --input Cargo.toml --output Cargo.toml --component ${SOURCE_NAME} ${lib_crates})
for _path in ${useless_crates}
do
	echo "    delete crate with path ${_path}"
	[ ! -d "${_path}" ] || rm -r "${_path}"
done


echo "==> cleaning useless cargo patches"
useless_patches=$(cargo tree 2>&1 >/dev/null | awk -f "${SCRIPT_DIR}"/useless_patch.awk)
"${SCRIPT_DIR}"/prune_patch.py --input Cargo.toml --output Cargo.toml ${useless_patches}


echo "==> cleaning local cargo checkouts"
for _file in $(cargo metadata --format-version 1 | jq -r '.packages[]["manifest_path"]|select(test("/tmp/build")|not)')
do
	_dir=$(dirname "${_file}")
	while [ -f "${_dir}/../Cargo.toml" ]
	do
		_dir=$(realpath "${_dir}/../")
	done

	if [ -f "${_dir}/Cargo.toml" ]
	then
		echo "   Cleaning ${_dir}"
		rm -rf "${_dir}"
	fi
done


echo "==> adding vendor code from cargo"
[ -d "${CARGO_CONFIG_DIR}" ] || mkdir "${CARGO_CONFIG_DIR}"
if [ -f "${CARGO_CONFIG_FILE}" ]
then
	printf "\n# for vendor-source\n" >> "${CARGO_CONFIG_FILE}"
else
	: > "${CARGO_CONFIG_FILE}"
fi
mkdir "${CARGO_VENDOR_DIR}"
cargo vendor --locked "${CARGO_VENDOR_DIR}" | tee -a "${CARGO_CONFIG_FILE}"


echo "==> building sphinx docs"
if [ -f ${SOURCE_NAME}/doc/conf.py ]
then
	mkdir -p ${SOURCE_NAME}/doc/_build
	sphinx-build ${SOURCE_NAME}/doc ${SOURCE_NAME}/doc/_build
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
tar -Jcf "${SOURCE_NAME}-${SOURCE_VERSION}.tar.xz" ${REPRODUCIBLE_OPTS} ${PROGRESS_OPTS} -C "${BUILD_DIR}" .
echo

:
