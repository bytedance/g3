#!/bin/sh

HTTP_PROXY="http://127.0.0.1:8080"
HTTPS_PROXY="https://g3proxy.local:8443"

python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTP_PROXY} -T http://httpbin.local
python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTP_PROXY} -T https://httpbin.local:9443 --no-auth --ca-cert ${SSL_CERT_FILE}
python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTP_PROXY} -T http://httpbin.local --no-auth --request-target-prefix https://httpbin.local:9443
python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTPS_PROXY} -T http://httpbin.local --proxy-ca-cert ${SSL_CERT_FILE}
python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTPS_PROXY} -T https://httpbin.local:9443 --no-auth --proxy-ca-cert ${SSL_CERT_FILE} --ca-cert ${SSL_CERT_FILE}
python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -x ${HTTPS_PROXY} -T http://httpbin.local --no-auth --proxy-ca-cert ${SSL_CERT_FILE} --request-target-prefix https://httpbin.local:9443

python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${HTTP_PROXY} -T http://httpbin.local
python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${HTTP_PROXY} -T https://httpbin.local:9443 --no-auth --ca-cert ${SSL_CERT_FILE}
python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${HTTPS_PROXY} -T http://httpbin.local
python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -x ${HTTPS_PROXY} -T https://httpbin.local:9443 --no-auth --ca-cert ${SSL_CERT_FILE}
