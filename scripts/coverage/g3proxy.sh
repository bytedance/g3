#!/bin/sh

set -e

SCRIPTS_DIR=$(dirname "$0")
PROJECT_DIR=$(realpath "${SCRIPTS_DIR}/../..")


TEST_NAME="g3proxy-ci"
. "${SCRIPTS_DIR}/enter.sh"

# build
cargo build -p g3proxy -p g3proxy-ctl -p g3proxy-ftp

all_binaries=$(find target/debug/ -maxdepth 1 -type f -perm /111 | awk '{print "-object "$0}')

# generate resource files
"${SCRIPTS_DIR}"/g3proxy/mkcert.sh

# run the tests
cargo test --all

all_objects=$(find target/debug/deps/ -type f -perm /111 -not -name "*.so" | awk '{print "-object "$0}')

# run integration tests

./target/debug/g3proxy -c "${SCRIPTS_DIR}/g3proxy/g3proxy.conf" -G ${TEST_NAME} &
proxy_pid=$!

all_http_proxies="http://127.0.0.1:10080 http://t1:toor@127.0.0.1:10082 http://t2:toor@127.0.0.1:10082 http://127.0.0.1:20080 http://127.0.0.1:20443 http://127.0.0.1:9001 http://127.0.0.1:9003"
all_socks_proxies="socks5h://127.0.0.1:11080 socks5h://127.0.0.1:11081 socks5h://t1:toor@127.0.0.1:11082 socks5h://127.0.0.1:21080 socks5h://127.0.0.1:21081 socks5h://127.0.0.1:9003"
partial_proxies="http://127.0.0.1:13128 http://127.0.0.1:10081 http://t3:toor@127.0.0.1:10082 http://127.0.0.1:20081 http://127.0.0.1:20082 http://127.0.0.1:20083 http://127.0.0.1:20084 socks5h://127.0.0.1:21082 socks5h://127.0.0.1:21083"
all_proxies="${all_http_proxies} ${all_socks_proxies} ${partial_proxies}"

sleep 2

##
echo "==== Update dynamic escapers"
./target/debug/g3proxy-ctl -G ${TEST_NAME} -p $proxy_pid escaper float10080 publish '{"type":"http","addr":"127.0.0.1:10080"}'
./target/debug/g3proxy-ctl -G ${TEST_NAME} -p $proxy_pid escaper float10443 publish '{"type":"https","addr":"127.0.0.1:10443", "tls_name": "g3proxy.local"}'
./target/debug/g3proxy-ctl -G ${TEST_NAME} -p $proxy_pid escaper float11080 publish '{"type":"socks5","addr":"127.0.0.1:11080"}'
./target/debug/g3proxy-ctl -G ${TEST_NAME} -p $proxy_pid escaper direct_lazy publish "{\"ipv4\": \"127.0.0.1\"}"

##
for resolver in main cares1 cares2 trust
do
	echo "==== Query directly on resolver ${resolver}"
	./target/debug/g3proxy-ctl -G ${TEST_NAME} -p $proxy_pid resolver ${resolver} query g3proxy.local
	./target/debug/g3proxy-ctl -G ${TEST_NAME} -p $proxy_pid resolver ${resolver} query httpbin.local
done

## tcp stream
echo "==== TCP Stream"
curl http://httpbin.local:9080/get

## tls stream
echo "==== TLS Stream"
curl https://httpbin.local:9443/get --cacert "${SCRIPTS_DIR}/g3proxy/rootCA.pem"

## SNI Proxy
echo "==== SNI Proxy"
curl https://httpbin.local:9443/get --cacert "${SCRIPTS_DIR}/g3proxy/rootCA.pem" --resolve httpbin.local:9443:[::1]

## http tproxy
echo "==== Http TProxy"
curl http://g3proxy.local:8080/get --resolve g3proxy.local:8080:[::1]

## http rproxy
echo "==== Http RProxy"
curl http://g3proxy.local:8080/get
curl https://g3proxy.local:8443/get --cacert "${SCRIPTS_DIR}/g3proxy/rootCA.pem"

## https proxy
echo "==== Https Proxy"
curl -x https://g3proxy.local:10443 http://httpbin.local/get --proxy-cacert "${SCRIPTS_DIR}/g3proxy/rootCA.pem"
curl -x https://g3proxy.local:9002 http://httpbin.local/get --proxy-cacert "${SCRIPTS_DIR}/g3proxy/rootCA.pem"

## socks4a proxy
echo "==== Socks4a Proxy"
curl -x socks4a://g3proxy.local:11080 http://httpbin.local/get

## httpbin
echo "==== httpbin"
for proxy in $all_proxies
do
	echo "-- ${proxy}"
	python3 "${PROJECT_DIR}/g3proxy/ci/httpbin/python3+requests/test_httpbin.py" -x ${proxy} -T http://httpbin.local || :
	python3 "${PROJECT_DIR}/g3proxy/ci/httpbin/python3+requests/test_httpbin.py" -x ${proxy} -T https://httpbin.local:9443 --ca-cert "${SCRIPTS_DIR}/g3proxy/rootCA.pem" || :
done

## DNS
echo "==== DNS"
for proxy in $all_socks_proxies
do
	echo "-- ${proxy}"
	"${SCRIPTS_DIR}/../test/socks5_dns_query.py" -x ${proxy} --dns-server 127.0.0.1 g3proxy.local httpbin.local -v || :
done

## FTP over HTTP
echo "==== FTP over HTTP"
for proxy in $all_http_proxies
do
	echo "-- ${proxy}"
	curl -x ${proxy} --upload-file "${SCRIPTS_DIR}/g3proxy/README.md" ftp://ftpuser:ftppass@127.0.0.1/README
	curl -x ${proxy} ftp://ftpuser:ftppass@127.0.0.1
	curl -x ${proxy} ftp://ftpuser:ftppass@127.0.0.1/README
done


./target/debug/g3proxy-ctl -G ${TEST_NAME} -p $proxy_pid offline
wait $proxy_pid

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

IGNORE_FLAGS="--ignore-filename-regex=.cargo --ignore-filename-regex=rustc --ignore-filename-regex=target/debug/build --ignore-filename-regex=g3bench --ignore-filename-regex=g3fcgen --ignore-filename-regex=g3tiles --ignore-filename-regex=demo"

echo "==== Coverage for libs ===="
cargo cov -- report --use-color --instr-profile="${PROF_DATA_FILE}" ${IGNORE_FLAGS} --ignore-filename-regex="g3proxy" ${all_binaries} ${all_objects}

echo "==== Coverage for all ===="
cargo cov -- report --use-color --instr-profile="${PROF_DATA_FILE}" ${IGNORE_FLAGS} ${all_binaries} ${all_objects}
