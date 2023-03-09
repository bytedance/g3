#!/bin/sh

set -e

SCRIPTS_DIR=$(dirname "$0")
PROJECT_DIR=$(realpath "${SCRIPTS_DIR}/../..")


TEST_NAME="unit-test"
. "${SCRIPTS_DIR}/enter.sh"

# build
cargo build --lib

all_binaries=$(find target/debug/ -maxdepth 1 -type f -perm /111 | awk '{print "-object "$0}')

# run the tests
cargo test --all

all_objects=$(find target/debug/deps/ -type f -perm /111 -not -name "*.so" | awk '{print "-object "$0}')


# get all profraw files generated in each test
profraw_files=$(find . -type f -regex ".*/${TEST_NAME}.*\.profraw")

# get indexed profile data file
cargo profdata -- merge -o "${PROF_DATA_FILE}" ${profraw_files}

# report to console

IGNORE_FLAGS="--ignore-filename-regex=.cargo --ignore-filename-regex=rustc --ignore-filename-regex=target/debug/build --ignore-filename-regex=g3bench --ignore-filename-regex=g3proxy  --ignore-filename-regex=g3rcgen --ignore-filename-regex=g3tiles --ignore-filename-regex=demo"

cargo cov -- report --use-color --instr-profile="${PROF_DATA_FILE}" ${IGNORE_FLAGS} ${all_binaries} ${all_objects}
