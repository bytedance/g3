#!/bin/sh

set -e

SCRIPTS_DIR=$(dirname "$0")
PROJECT_DIR=$(realpath "${SCRIPTS_DIR}/../..")


TEST_NAME="bench-ci"
. "${SCRIPTS_DIR}/enter.sh"

# build
cargo build -p g3bench -p g3mkcert -p g3proxy -p g3proxy-ctl

all_binaries=$(find target/debug/ -maxdepth 1 -type f -perm /111 | awk '{print "-object "$0}')

# run the tests
cargo test --all

all_objects=$(find target/debug/deps/ -type f -perm /111 -not -name "*.so" | awk '{print "-object "$0}')

# generate resource files
"${SCRIPTS_DIR}"/g3bench/mkcert.sh

# start g3proxy
"${PROJECT_DIR}"/target/debug/g3proxy -c "${SCRIPTS_DIR}"/g3bench/g3proxy.yaml -G ${TEST_NAME} &
PROXY_PID=$!

# start nginx
[ -d /tmp/nginx ] || mkdir /tmp/nginx
/usr/sbin/nginx -c "${PROJECT_DIR}"/scripts/coverage/g3bench/nginx.conf

# run g3bench integration tests

export SSL_CERT_FILE="${SCRIPTS_DIR}/g3bench/rootCA.pem"

g3bench()
{
	"${PROJECT_DIR}"/target/debug/g3bench --log-error 1 "$@"
}

set -x

. ${SCRIPTS_DIR}/g3bench/target_dns.sh
. ${SCRIPTS_DIR}/g3bench/target_h1.sh
. ${SCRIPTS_DIR}/g3bench/target_h2.sh
. ${SCRIPTS_DIR}/g3bench/target_keyless_openssl.sh
. ${SCRIPTS_DIR}/g3bench/target_openssl.sh
. ${SCRIPTS_DIR}/g3bench/target_rustls.sh

set +x

"${PROJECT_DIR}"/target/debug/g3proxy-ctl -G ${TEST_NAME} -p $PROXY_PID offline

NGINX_PID=$(cat /tmp/nginx.pid)
kill -INT $NGINX_PID


# get all profraw files generated in each test
profraw_files=$(find . -type f -regex ".*/${TEST_NAME}.*\.profraw")

# get indexed profile data file
cargo profdata -- merge -o "${PROF_DATA_FILE}" ${profraw_files}

# report to console

IGNORE_FLAGS="--ignore-filename-regex=.cargo \
    --ignore-filename-regex=rustc \
    --ignore-filename-regex=target/debug/build \
    --ignore-filename-regex=g3mkcert \
    --ignore-filename-regex=g3fcgen \
    --ignore-filename-regex=g3proxy \
    --ignore-filename-regex=g3tiles \
    --ignore-filename-regex=g3keymess \
    --ignore-filename-regex=g3iploc"

echo "==== Coverage for libs ===="
cargo cov -- report --use-color --instr-profile="${PROF_DATA_FILE}" ${IGNORE_FLAGS} --ignore-filename-regex="g3bench" ${all_binaries} ${all_objects}

echo "==== Coverage for all ===="
cargo cov -- report --use-color --instr-profile="${PROF_DATA_FILE}" ${IGNORE_FLAGS} ${all_binaries} ${all_objects}

cargo cov -- export --format=lcov --instr-profile="${PROF_DATA_FILE}" ${IGNORE_FLAGS} ${all_binaries} ${all_objects} > output.lcov
