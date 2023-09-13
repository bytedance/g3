#!/bin/sh

set -e

SCRIPTS_DIR=$(dirname "$0")
PROJECT_DIR=$(realpath "${SCRIPTS_DIR}/..")

usage () {
	cat << EOF
Usage: $0 [options]

Options:
  -s Builds the source package
  -h Show usage message

EOF
}

create_orig_tar () {
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
}

while getopts hs f
do
	case $f in
	h)
		usage;exit 0;;
	s)
		BUILD_SOURCE="1";;
	\?)
		usage;exit 1;;
	esac
done
shift $((OPTIND - 1))

cd "${PROJECT_DIR}"

CODENAME=$(lsb_release -c -s)

echo "Finalize debian/changelog"
sed -i s/UNRELEASED/${CODENAME}/ debian/changelog

BUILD_FLAGS="-uc"

if [ -n "${BUILD_SOURCE}" ]
then
	create_orig_tar

	BUILD_FLAGS="-us ${BUILD_FLAGS}"
else
	BUILD_FLAGS="-b ${BUILD_FLAGS}"
fi

echo "Building"
export RUSTFLAGS="--remap-path-prefix ${HOME}=~"

dpkg-buildpackage ${BUILD_FLAGS}
