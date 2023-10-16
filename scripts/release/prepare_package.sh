#!/bin/bash

set -e

SCRIPT_DIR=$(dirname $(realpath $0))

PACKAGE=$1
VERSION=$2
PKG_VERSION=$(echo ${VERSION} | tr '-' '.')

cd ${SCRIPT_DIR}/../../

DCH_FILE=${PACKAGE}/debian/changelog
echo "Update ${DCH_FILE}"

DCH_TIME=$(LANG=en_US date +"%a, %d %b %Y %H:%M:%S %z")

cat << EOF > ${DCH_FILE}
${PACKAGE} (${PKG_VERSION}-1) UNRELEASED; urgency=medium

  * New upstream release.

 -- ${PACKAGE^} Maintainers <${PACKAGE}-maintainers@devel.machine>  ${DCH_TIME}
EOF

SPEC_FILE=${PACKAGE}/${PACKAGE}.spec
echo "Update ${SPEC_FILE}"

SPEC_TIME=$(LANG=en_US date +"%a %b %d %Y")

TMP_FILE=${SPEC_FILE}.tmp
awk -f scripts/release/prepare_rpm_spec.awk -v VERSION="${PKG_VERSION}" ${SPEC_FILE} > ${TMP_FILE}
cat << EOF >> ${TMP_FILE}
%changelog
* ${SPEC_TIME} ${PACKAGE^} Maintainers <${PACKAGE}-maintainers@devel.machine> - ${PKG_VERSION}-1
- New upstream release
EOF
mv ${TMP_FILE} ${SPEC_FILE}

