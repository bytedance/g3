#!/bin/sh

HTTP_PROXY="http://127.0.0.1:8080"

python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTP_PROXY} -T http://httpbin.local

python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${HTTP_PROXY} -T http://httpbin.local


SOCKS5_PROXY="socks5h://127.0.0.1:1080"
SOCKS4_PROXY="socks4a://127.0.0.1:1080"

python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${SOCKS5_PROXY} -T http://httpbin.local
python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${SOCKS4_PROXY} -T http://httpbin.local

python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${SOCKS5_PROXY} -T http://httpbin.local

python3 "${PROJECT_DIR}/scripts/test/socks5_dns_query.py" -x ${SOCKS5_PROXY} --dns-server 127.0.0.1 g3proxy.local httpbin.local -v


SOCKS5_PROXY="socks5h://127.0.0.1:1081"
SOCKS4_PROXY="socks4a://127.0.0.1:1081"

python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${SOCKS5_PROXY} -T http://httpbin.local
python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${SOCKS4_PROXY} -T http://httpbin.local

python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${SOCKS5_PROXY} -T http://httpbin.local

python3 "${PROJECT_DIR}/scripts/test/socks5_dns_query.py" -x ${SOCKS5_PROXY} --dns-server 127.0.0.1 g3proxy.local httpbin.local -v
