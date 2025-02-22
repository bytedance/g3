
g3bench rustls httpbin.local:9443 --tls-ca-cert "${TEST_CA_CERT_FILE}"

g3bench rustls 127.0.0.1:9443 --tls-name httpbin.local --tls-ca-cert "${TEST_CA_CERT_FILE}"
