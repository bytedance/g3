#!/bin/sh

set -e

SCRIPTS_DIR=$(dirname "$0")
PROJECT_DIR=$(realpath "${SCRIPTS_DIR}/../..")


TEST_NAME="g3proxy-ci"
. "${SCRIPTS_DIR}/enter.sh"

# build
cargo build -p g3proxy -p g3proxy-ctl -p g3proxy-ftp -p g3mkcert

all_binaries=$(find target/debug/ -maxdepth 1 -type f -perm /111 | awk '{print "-object "$0}')

# run the tests
cargo test --all

all_objects=$(find target/debug/deps/ -type f -perm /111 -not -name "*.so" | awk '{print "-object "$0}')

# generate resource files
"${SCRIPTS_DIR}"/g3proxy/mkcert.sh

export SSL_CERT_FILE="${SCRIPTS_DIR}/g3proxy/rootCA.pem"

# run g3proxy integration tests

g3proxy_ctl()
{
	"${PROJECT_DIR}"/target/debug/g3proxy-ctl -G ${TEST_NAME} -p $PROXY_PID "$@"
}

set -x

for dir in $(find "${SCRIPTS_DIR}/g3proxy/" -type d | sort)
do
	[ -f "${dir}/g3proxy.yaml" ] || continue

	echo "=== ${dir}"

	"${PROJECT_DIR}"/target/debug/g3proxy -c "${dir}/g3proxy.yaml" -G ${TEST_NAME} &
	PROXY_PID=$!

	sleep 2

	[ -f "${dir}/testcases.sh" ] || continue
	. "${dir}/testcases.sh"

	g3proxy_ctl offline
	wait $PROXY_PID
done

set +x

## g3proxy-ftp

echo "==== g3proxy-ftp"
./target/debug/g3proxy-ftp -u ftpuser -p ftppass 127.0.0.1 list
./target/debug/g3proxy-ftp -u ftpuser -p ftppass 127.0.0.1 put --file "${SCRIPTS_DIR}/g3proxy/README.md" README
./target/debug/g3proxy-ftp -u ftpuser -p ftppass 127.0.0.1 get README
./target/debug/g3proxy-ftp -u ftpuser -p ftppass 127.0.0.1 del README

# get all profraw files generated in each test
profraw_files=$(find . -type f -regex ".*/${TEST_NAME}.*\.profraw")

# get indexed profile data file
cargo profdata -- merge -o "${PROF_DATA_FILE}" ${profraw_files}

# report to console

IGNORE_FLAGS="--ignore-filename-regex=.cargo \
    --ignore-filename-regex=rustc \
    --ignore-filename-regex=target/debug/build \
    --ignore-filename-regex=g3bench \
    --ignore-filename-regex=g3mkcert \
    --ignore-filename-regex=g3fcgen \
    --ignore-filename-regex=g3tiles \
    --ignore-filename-regex=g3keymess \
    --ignore-filename-regex=g3iploc"

echo "==== Coverage for libs ===="
cargo cov -- report --use-color --instr-profile="${PROF_DATA_FILE}" ${IGNORE_FLAGS} --ignore-filename-regex="g3proxy" ${all_binaries} ${all_objects}

echo "==== Coverage for all ===="
cargo cov -- report --use-color --instr-profile="${PROF_DATA_FILE}" ${IGNORE_FLAGS} ${all_binaries} ${all_objects}

cargo cov -- export --format=lcov --instr-profile="${PROF_DATA_FILE}" ${IGNORE_FLAGS} ${all_binaries} ${all_objects} > output.lcov
