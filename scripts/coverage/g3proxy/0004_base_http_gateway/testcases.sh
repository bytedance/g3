#!/bin/sh

python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -T http://httpbin.local:8080 --no-auth
python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -T https://httpbin.local:8443 --no-auth --ca-cert "${TEST_CA_CERT_FILE}"
python3 "${PROJECT_DIR}/g3proxy/ci/python3+curl/test_httpbin.py" -T https://httpbin.local:9443 --no-auth --ca-cert "${TEST_CA_CERT_FILE}"

python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -T http://httpbin.local:8080 --no-auth
python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -T https://httpbin.local:8443 --no-auth
python3 "${PROJECT_DIR}/g3proxy/ci/python3+requests/test_httpbin.py" -T https://httpbin.local:9443 --no-auth
