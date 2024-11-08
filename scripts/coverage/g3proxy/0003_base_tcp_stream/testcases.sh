#!/bin/sh

python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -T http://httpbin.local:8080

python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -T https://httpbin.local:8443 --ca-cert ${SSL_CERT_FILE}
