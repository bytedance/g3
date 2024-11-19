#!/bin/sh


test_http_proxy_http_forward()
{
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTP_PROXY} -T http://httpbin.local

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${HTTP_PROXY} -T http://httpbin.local
}


test_http_proxy_ftp_over_http()
{
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_ftp_over_http.py" -x ${HTTP_PROXY} -T ftp://ftpuser:ftppass@127.0.0.1
}


test_https_proxy_http_forward()
{
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTPS_PROXY} -T http://httpbin.local --proxy-ca-cert ${SSL_CERT_FILE}

	python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${HTTPS_PROXY} -T http://httpbin.local
}


test_https_proxy_ftp_over_http()
{
	python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_ftp_over_http.py" -x ${HTTPS_PROXY} -T ftp://ftpuser:ftppass@127.0.0.1 --proxy-ca-cert ${SSL_CERT_FILE}
}


HTTP_PROXY="http://127.0.0.1:8080"
test_http_proxy_http_forward
test_http_proxy_ftp_over_http


HTTPS_PROXY="https://g3proxy.local:8443"
test_https_proxy_http_forward
test_https_proxy_ftp_over_http


HTTPS_PROXY="https://g3proxy.local:9443"
test_https_proxy_http_forward
test_https_proxy_ftp_over_http
