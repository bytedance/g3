#!/bin/sh

partial_proxies="http://127.0.0.1:13128"
all_proxies="${partial_proxies}"

##
echo "==== Update dynamic escapers"
./target/debug/g3proxy-ctl -G ${TEST_NAME} -p $PROXY_PID escaper direct_lazy publish "{\"ipv4\": \"127.0.0.1\"}"

## httpbin
echo "==== httpbin"
for proxy in $all_proxies
do
	echo "-- ${proxy}"
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${proxy} -T http://httpbin.local || :
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${proxy} -T https://httpbin.local:2443 --ca-cert "${SCRIPTS_DIR}/g3proxy/rootCA.pem" || :
done
