#!/usr/bin/sh

set -e

SCRIPT_DIR=$(dirname $0)
INCORPORATE_DIR="${SCRIPT_DIR}/../../../third_party/boringssl"

cd ${INCORPORATE_DIR}

python3 util/generate_build_files.py cmake
