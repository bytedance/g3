#!/bin/sh

python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -T http://httpbin.local:8080 --resolve httpbin.local:8080:[::1]
python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -T https://httpbin.local:8443 --resolve httpbin.local:8443:[::1] --ca-cert ${SSL_CERT_FILE}
