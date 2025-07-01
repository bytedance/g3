# generate resource files
"${RUN_DIR}"/mkcert.sh

# start g3statsd
"${PROJECT_DIR}"/target/debug/g3statsd -c "${RUN_DIR}"/g3statsd.yaml -G ${TEST_NAME} &
STATSD_PID=$!

# run g3keymess integration tests

g3keymess_ctl()
{
	"${PROJECT_DIR}"/target/debug/g3keymess-ctl -G ${TEST_NAME} -p $KEYSERVER_PID "$@"
}

g3bench()
{
	"${PROJECT_DIR}"/target/debug/g3bench --no-progress-bar --log-error 1 "$@"
}

test_rsa()
{
	TEST_RSA_KEY_FILE="${RUN_DIR}/keys/rsa2048.key"
	TEST_RSA_CERT_FILE="${RUN_DIR}/rsa2048.crt"

	for hash in sha256 sha384 sha512
	do
		payload=$("${hash}sum" "${TEST_RSA_KEY_FILE}" | awk '{print $1}')
		g3bench keyless cloudflare --no-tls --target 127.0.0.1:1300 --key "${TEST_RSA_KEY_FILE}" --sign --digest-type $hash --verify "${payload}"
		g3bench keyless cloudflare --no-tls --target 127.0.0.1:1300 --key "${TEST_RSA_KEY_FILE}" --sign --digest-type $hash --rsa-padding PSS --verify "${payload}"
	done

	TO_DECRYPT_DATA=$(g3bench keyless openssl --key "${TEST_RSA_KEY_FILE}" --encrypt "abcdef" --no-summary --dump-result)
	g3bench keyless cloudflare --no-tls --target 127.0.0.1:1300 --key "${TEST_RSA_KEY_FILE}" --decrypt --verify --verify-data "abcdef" "${TO_DECRYPT_DATA}"
}

test_ec()
{
	TEST_EC_KEY_FILE="${RUN_DIR}/keys/ec256.key"
	TEST_EC_CERT_FILE="${RUN_DIR}/ec256.crt"

	for hash in sha256 sha384 sha512
	do
		payload=$("${hash}sum" "${TEST_RSA_KEY_FILE}" | awk '{print $1}')
		g3bench keyless cloudflare --no-tls --target 127.0.0.1:1300 --key "${TEST_EC_KEY_FILE}" --sign --digest-type $hash --verify "${payload}"
	done
}

set -x

for dir in $(ls "${PROJECT_DIR}"/g3keymess/examples)
do
	example_dir="${PROJECT_DIR}/g3keymess/examples/${dir}"
	[ -d "${example_dir}" ] || continue

	"${PROJECT_DIR}"/target/debug/g3keymess -c "${example_dir}" -t
done

for dir in $(find "${RUN_DIR}/" -type d | sort)
do
	[ -f "${dir}/g3keymess.yaml" ] || continue

	echo "=== ${dir}"
	date

	"${PROJECT_DIR}"/target/debug/g3keymess -c "${dir}/g3keymess.yaml" -G ${TEST_NAME} &
	KEYSERVER_PID=$!

	sleep 2

	test_rsa
	test_ec

	g3keymess_ctl offline
	wait $KEYSERVER_PID
done

set +x

kill -INT $STATSD_PID
