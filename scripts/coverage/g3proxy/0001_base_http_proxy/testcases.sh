#!/bin/sh


test_http_proxy_http_forward()
{
	date

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTP_PROXY} -T http://httpbin.local
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTP_PROXY} -T http://127.0.0.1

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${HTTP_PROXY} -T http://httpbin.local
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${HTTP_PROXY} -T http://127.0.0.1
}


test_http_proxy_http_connect()
{
	date

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTP_PROXY} --proxy-tunnel -T http://httpbin.local
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTP_PROXY} --proxy-tunnel -T http://127.0.0.1
}


test_http_easy_proxy_http_forward()
{
	date

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -T ${HTTP_PROXY}/.well-known/easy-proxy/http/httpbin.local/80/
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -T ${HTTP_PROXY}/.well-known/easy-proxy/http/127.0.0.1/80/

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -T ${HTTP_PROXY}/.well-known/easy-proxy/http/httpbin.local/80/
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -T ${HTTP_PROXY}/.well-known/easy-proxy/http/127.0.0.1/80/
}


test_http_masque_http_forward()
{
	date

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTP_PROXY} --proxy-masque -T http://httpbin.local/
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTP_PROXY} --proxy-masque -T http://127.0.0.1/
}


test_http_proxy_ftp_over_http()
{
	date

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_ftp_over_http.py" -x ${HTTP_PROXY} -T ftp://ftpuser:ftppass@127.0.0.1
}


test_https_proxy_http_forward()
{
	date

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTPS_PROXY} -T http://httpbin.local --proxy-ca-cert "${TEST_CA_CERT_FILE}"

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${HTTPS_PROXY} -T http://httpbin.local
}


test_https_easy_proxy_http_forward()
{
	date

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -T ${HTTPS_PROXY}/.well-known/easy-proxy/http/httpbin.local/80/ --ca-cert "${TEST_CA_CERT_FILE}"

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -T ${HTTPS_PROXY}/.well-known/easy-proxy/http/httpbin.local/80/
}


test_https_masque_http_forward()
{
	date

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTPS_PROXY} --proxy-masque -T http://httpbin.local/ --ca-cert "${TEST_CA_CERT_FILE}"
}


test_https_proxy_ftp_over_http()
{
	date

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_ftp_over_http.py" -x ${HTTPS_PROXY} -T ftp://ftpuser:ftppass@127.0.0.1 --proxy-ca-cert "${TEST_CA_CERT_FILE}"
}


HTTP_PROXY="http://127.0.0.1:8080"
test_http_proxy_http_forward
test_http_proxy_http_connect
test_http_proxy_ftp_over_http
test_http_easy_proxy_http_forward
test_http_masque_http_forward


HTTP_PROXY="http://[::1]:8080"
test_http_proxy_http_forward
test_http_proxy_ftp_over_http
test_http_easy_proxy_http_forward
test_http_masque_http_forward


for port in 8443 8444 9443
do
	HTTPS_PROXY="https://g3proxy.local:${port}"
	test_https_proxy_http_forward
	test_https_proxy_ftp_over_http
	test_https_easy_proxy_http_forward
	test_https_masque_http_forward
done
