# generate resource files
"${RUN_DIR}"/mkcert.sh

# start g3proxy
"${PROJECT_DIR}"/target/debug/g3proxy -c "${RUN_DIR}"/g3proxy.yaml -G ${TEST_NAME} &
PROXY_PID=$!

# start nginx
[ -d /tmp/nginx ] || mkdir /tmp/nginx
/usr/sbin/nginx -c "${PROJECT_DIR}"/scripts/coverage/g3bench/nginx.conf

# run g3bench integration tests

export SSL_CERT_FILE="${RUN_DIR}/rootCA.pem"
export RSA_KEY_FILE="${RUN_DIR}/rootCA-RSA-key.pem"
export EC_KEY_FILE="${RUN_DIR}/rootCA-EC-key.pem"

g3bench()
{
	"${PROJECT_DIR}"/target/debug/g3bench --log-error 1 "$@"
}

set -x

. ${RUN_DIR}/target_dns.sh
. ${RUN_DIR}/target_h1.sh
. ${RUN_DIR}/target_h2.sh
. ${RUN_DIR}/target_keyless_openssl.sh
. ${RUN_DIR}/target_openssl.sh
. ${RUN_DIR}/target_rustls.sh

set +x

"${PROJECT_DIR}"/target/debug/g3proxy-ctl -G ${TEST_NAME} -p $PROXY_PID offline

NGINX_PID=$(cat /tmp/nginx.pid)
kill -INT $NGINX_PID
