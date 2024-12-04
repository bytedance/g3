
g3bench openssl httpbin.local:9443 --tls-ca-cert ${SSL_CERT_FILE}

g3bench openssl 127.0.0.1:9443 --tls-name httpbin.local --tls-ca-cert ${SSL_CERT_FILE}
