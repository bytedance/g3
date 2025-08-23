
g3bench h2 https://httpbin.local:2443/get --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"

g3bench h2 https://httpbin.local:2443/get -H "Accept: application/json" --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"

g3bench h2 https://httpbin.local:2443/get -x http://t1:toor@g3proxy.local:8080 --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"

g3bench h2 https://httpbin.local:2443/get -x https://t1:toor@g3proxy.local:8443 --proxy-tls-ca-cert "${TEST_CA_CERT_FILE}" --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"

g3bench h2 https://httpbin.local:2443/get -x socks5h://t1:toor@g3proxy.local:1080 --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"
