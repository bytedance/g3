#!/bin/sh

all_http_proxies="http://127.0.0.1:10080 http://t1:toor@127.0.0.1:10082 http://t2:toor@127.0.0.1:10082 http://127.0.0.1:20080 http://127.0.0.1:20443 http://127.0.0.1:9001 http://127.0.0.1:9003"
all_socks_proxies="socks5h://127.0.0.1:11080 socks5h://127.0.0.1:11081 socks5h://t1:toor@127.0.0.1:11082 socks5h://127.0.0.1:21080 socks5h://127.0.0.1:21081 socks5h://127.0.0.1:9003"
partial_proxies="http://127.0.0.1:13128 http://127.0.0.1:10081 http://t3:toor@127.0.0.1:10082 http://127.0.0.1:20081 http://127.0.0.1:20082 http://127.0.0.1:20083 http://127.0.0.1:20084 socks5h://127.0.0.1:21082 socks5h://127.0.0.1:21083"
all_proxies="${all_http_proxies} ${all_socks_proxies} ${partial_proxies}"

##
echo "==== Update dynamic escapers"
./target/debug/g3proxy-ctl -G ${TEST_NAME} -p $PROXY_PID escaper float10080 publish '{"type":"http","addr":"127.0.0.1:10080"}'
./target/debug/g3proxy-ctl -G ${TEST_NAME} -p $PROXY_PID escaper float10443 publish '{"type":"https","addr":"127.0.0.1:10443", "tls_name": "g3proxy.local"}'
./target/debug/g3proxy-ctl -G ${TEST_NAME} -p $PROXY_PID escaper float11080 publish '{"type":"socks5","addr":"127.0.0.1:11080"}'
./target/debug/g3proxy-ctl -G ${TEST_NAME} -p $PROXY_PID escaper direct_lazy publish "{\"ipv4\": \"127.0.0.1\"}"

## https proxy
echo "==== Https Proxy"
curl -x https://g3proxy.local:10443 http://httpbin.local/get --proxy-cacert "${SCRIPTS_DIR}/g3proxy/rootCA.pem"
curl -x https://g3proxy.local:9002 http://httpbin.local/get --proxy-cacert "${SCRIPTS_DIR}/g3proxy/rootCA.pem"

## httpbin
echo "==== httpbin"
for proxy in $all_proxies
do
	echo "-- ${proxy}"
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${proxy} -T http://httpbin.local || :
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${proxy} -T https://httpbin.local:9443 --ca-cert "${SCRIPTS_DIR}/g3proxy/rootCA.pem" || :
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
