#!/bin/sh

# Exercise username-params-to-escaper mapping for both HTTP and SOCKS5.
# We intentionally point mapping ports to 9 to avoid external dependencies.

date

# HTTP proxy with proxy-basic auth username carrying params
HTTP_PROXY="http://user+label1=foo+opt=o123:pass@127.0.0.1:13080"
python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${HTTP_PROXY} -T http://httpbin.local --no-auth || :
python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${HTTP_PROXY} -T https://httpbin.local:2443 --no-auth --ca-cert "${TEST_CA_CERT_FILE}" || :

# Also hit the error branch by using an invalid hierarchy (label2 without label1)
HTTP_PROXY="http://user+label2=bar:pass@127.0.0.1:13080"
python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTP_PROXY} -T http://httpbin.local --no-auth || :

# SOCKS5 proxy with username carrying params
SOCKS5_PROXY="socks5h://user+opt=only:pass@127.0.0.1:11081"
python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${SOCKS5_PROXY} -T http://httpbin.local --no-auth || :
python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${SOCKS5_PROXY} -T https://httpbin.local:2443 --no-auth --ca-cert "${TEST_CA_CERT_FILE}" || :

