
# get influxdb auth token
[ -n "${INFLUXDB3_AUTH_TOKEN}" ] || INFLUXDB3_AUTH_TOKEN=$(curl -X POST http://127.0.0.1:8181/api/v3/configure/token/admin | jq ".token" -r)
export INFLUXDB3_AUTH_TOKEN

# run g3statsd integration tests

g3statsd_ctl()
{
	"${PROJECT_DIR}"/target/debug/g3statsd-ctl -G ${TEST_NAME} -p $STATSD_PID "$@"
}

set -x

for dir in $(ls "${PROJECT_DIR}/g3statsd/examples")
do
	example_dir="${PROJECT_DIR}/g3statsd/examples/${dir}"
	[ -d "${example_dir}" ] || continue

	"${PROJECT_DIR}"/target/debug/g3statsd -c "${example_dir}" -t
done

for dir in $(find "${RUN_DIR}/" -type d | sort)
do
	[ -f "${dir}/g3statsd.yaml" ] || continue

	echo "=== ${dir}"

	"${PROJECT_DIR}"/target/debug/g3statsd -c "${dir}/g3statsd.yaml" -G ${TEST_NAME} &
	STATSD_PID=$!

	sleep 2

	[ -f "${dir}/testcases.sh" ] || continue
	TESTCASE_DIR=${dir}
	. "${dir}/testcases.sh"

	g3statsd_ctl offline
	wait $STATSD_PID
done

set +x
