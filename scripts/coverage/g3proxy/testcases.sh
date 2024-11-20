#!/bin/sh

all_http_proxies="http://127.0.0.1:10080"
all_socks_proxies="socks5h://127.0.0.1:11080"
partial_proxies="http://127.0.0.1:13128"
all_proxies="${all_http_proxies} ${all_socks_proxies} ${partial_proxies}"

##
echo "==== Update dynamic escapers"
./target/debug/g3proxy-ctl -G ${TEST_NAME} -p $PROXY_PID escaper direct_lazy publish "{\"ipv4\": \"127.0.0.1\"}"

## httpbin
echo "==== httpbin"
for proxy in $all_proxies
do
	echo "-- ${proxy}"
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${proxy} -T http://httpbin.local || :
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${proxy} -T https://httpbin.local:9443 --ca-cert "${SCRIPTS_DIR}/g3proxy/rootCA.pem" || :
done
