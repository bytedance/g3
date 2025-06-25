#!/bin/sh


test_http_proxy_https_connect()
{
	date

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTP_PROXY} -T https://httpbin.local:9443 --no-auth --ca-cert "${TEST_CA_CERT_FILE}"
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTP_PROXY} -T https://httpbin.local:2443 --no-auth --ca-cert "${TEST_CA_CERT_FILE}"

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${HTTP_PROXY} -T https://httpbin.local:9443 --no-auth --ca-cert "${TEST_CA_CERT_FILE}"
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${HTTP_PROXY} -T https://httpbin.local:2443 --no-auth --ca-cert "${TEST_CA_CERT_FILE}"
}


test_http_proxy_https_forward()
{
	date

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTP_PROXY} -T http://httpbin.local --no-auth --request-target-prefix https://httpbin.local:9443
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTP_PROXY} -T http://httpbin.local --no-auth --request-target-prefix https://httpbin.local:2443
}


test_http_easy_proxy_https_forward()
{
	date

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -T ${HTTP_PROXY}/.well-known/easy-proxy/https/httpbin.local/9443/ --no-auth
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -T ${HTTP_PROXY}/.well-known/easy-proxy/https/httpbin.local/2443/ --no-auth
}


test_http_proxy_h2()
{
	date

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin_h2.py" -x ${HTTP_PROXY} -T https://httpbin.local:2443 --ca-cert "${TEST_CA_CERT_FILE}"
}


test_https_proxy_https_connect()
{
	date

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTPS_PROXY} -T https://httpbin.local:9443 --no-auth --proxy-ca-cert "${TEST_CA_CERT_FILE}" --ca-cert "${TEST_CA_CERT_FILE}"
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTPS_PROXY} -T https://httpbin.local:2443 --no-auth --proxy-ca-cert "${TEST_CA_CERT_FILE}" --ca-cert "${TEST_CA_CERT_FILE}"

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${HTTPS_PROXY} -T https://httpbin.local:9443 --no-auth --ca-cert "${TEST_CA_CERT_FILE}"
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${HTTPS_PROXY} -T https://httpbin.local:2443 --no-auth --ca-cert "${TEST_CA_CERT_FILE}"
}


test_https_proxy_https_forward()
{
	date

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTPS_PROXY} -T http://httpbin.local --no-auth --proxy-ca-cert "${TEST_CA_CERT_FILE}" --request-target-prefix https://httpbin.local:9443
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTPS_PROXY} -T http://httpbin.local --no-auth --proxy-ca-cert "${TEST_CA_CERT_FILE}" --request-target-prefix https://httpbin.local:2443
}


test_https_easy_proxy_https_forward()
{
	date

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -T ${HTTPS_PROXY}/.well-known/easy-proxy/https/httpbin.local/9443/ --no-auth --ca-cert "${TEST_CA_CERT_FILE}"
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -T ${HTTPS_PROXY}/.well-known/easy-proxy/https/httpbin.local/2443/ --no-auth --ca-cert "${TEST_CA_CERT_FILE}"
}


test_https_proxy_h2()
{
	date

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin_h2.py" -x ${HTTPS_PROXY} -T https://httpbin.local:2443 --proxy-ca-cert "${TEST_CA_CERT_FILE}" --ca-cert "${TEST_CA_CERT_FILE}"
}


test_socks5_proxy_https()
{
	date

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${SOCKS5_PROXY} -T https://httpbin.local:9443 --no-auth --ca-cert "${TEST_CA_CERT_FILE}"
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${SOCKS5_PROXY} -T https://httpbin.local:2443 --no-auth --ca-cert "${TEST_CA_CERT_FILE}"

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${SOCKS5_PROXY} -T https://httpbin.local:9443 --no-auth --ca-cert "${TEST_CA_CERT_FILE}"
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${SOCKS5_PROXY} -T https://httpbin.local:2443 --no-auth --ca-cert "${TEST_CA_CERT_FILE}"
}


test_socks4_proxy_https()
{
	date

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${SOCKS4_PROXY} -T https://httpbin.local:9443 --no-auth --ca-cert "${TEST_CA_CERT_FILE}"
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${SOCKS4_PROXY} -T https://httpbin.local:2443 --no-auth --ca-cert "${TEST_CA_CERT_FILE}"
}


HTTP_PROXY="http://127.0.0.1:8080"
test_http_proxy_http_forward
# FTP not supported in proxy escaper
#test_http_proxy_ftp_over_http
test_http_easy_proxy_https_forward
test_http_proxy_https_connect
test_http_proxy_https_forward


HTTPS_PROXY="https://g3proxy.local:8443"
test_https_proxy_http_forward
test_https_proxy_ftp_over_http
test_https_easy_proxy_https_forward
test_https_proxy_https_connect
test_https_proxy_https_forward


SOCKS5_PROXY="socks5h://127.0.0.1:1080"
test_socks5_proxy_http
test_socks5_proxy_https


SOCKS4_PROXY="socks4a://127.0.0.1:1080"
test_socks4_proxy_http
test_socks4_proxy_https
