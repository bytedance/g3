# generate resource files
"${RUN_DIR}"/mkcert.sh

# start g3proxy
"${PROJECT_DIR}"/target/debug/g3proxy -c "${RUN_DIR}"/g3proxy.yaml -G "${TEST_NAME}" &
PROXY_PID=$!

# start nginx
[ -d /tmp/nginx ] || mkdir /tmp/nginx
/usr/sbin/nginx -c "${PROJECT_DIR}"/scripts/coverage/g3bench/nginx.conf

# start g3statsd
[ -n "${INFLUX_TOKEN}" ] || INFLUX_TOKEN=$(curl -X POST http://127.0.0.1:8181/api/v3/configure/token/admin | jq ".token" -r)
export INFLUX_TOKEN
"${PROJECT_DIR}"/target/debug/g3statsd -c "${RUN_DIR}"/g3statsd.yaml -G ${TEST_NAME} &
STATSD_PID=$!

# run g3bench integration tests

export TEST_CA_CERT_FILE="${RUN_DIR}/rootCA.pem"
export TEST_RSA_KEY_FILE="${RUN_DIR}/rootCA-RSA-key.pem"
export TEST_RSA_CERT_FILE="${RUN_DIR}/rootCA-RSA.pem"
export TEST_EC_KEY_FILE="${RUN_DIR}/rootCA-EC-key.pem"

g3bench()
{
	"${PROJECT_DIR}"/target/debug/g3bench --no-progress-bar --log-error 1 "$@"
}

set -x

. "${RUN_DIR}"/target_dns.sh
. "${RUN_DIR}"/target_h1.sh
. "${RUN_DIR}"/target_h2.sh
. "${RUN_DIR}"/target_keyless_openssl.sh
. "${RUN_DIR}"/target_openssl.sh
. "${RUN_DIR}"/target_rustls.sh
. "${RUN_DIR}"/target_thrift_tcp.sh

set +x

"${PROJECT_DIR}"/target/debug/g3proxy-ctl -G "${TEST_NAME}" -p $PROXY_PID offline

kill -INT $STATSD_PID
NGINX_PID=$(cat /tmp/nginx.pid)
kill -INT $NGINX_PID
