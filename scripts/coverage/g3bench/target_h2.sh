
# GET

URL=https://httpbin.local:2443/get

g3bench h2 "${URL}" --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"

g3bench h2 "${URL}" -H "Accept: application/json" --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"

g3bench h2 "${URL}" -x http://t1:toor@g3proxy.local:8080 --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"

g3bench h2 "${URL}" -x https://t1:toor@g3proxy.local:8443 --proxy-tls-ca-cert "${TEST_CA_CERT_FILE}" --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"

g3bench h2 "${URL}" -x socks5h://t1:toor@g3proxy.local:1080 --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"

# POST

URL=https://httpbin.local:2443/post

g3bench h2 "${URL}" --method POST --payload 31323334 --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"
g3bench h2 "${URL}" --method POST --payload 31323334 --binary --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"
g3bench h2 "${URL}" --method POST --payload name=foo -H "Content-Type: application/x-www-form-urlencoded" --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"
