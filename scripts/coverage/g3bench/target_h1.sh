
# Http

test_http_get()
{
	URL=$1

	g3bench h1 "${URL}" --ok-status 200
	g3bench h1 "${URL}" -H "Accept: application/json" --ok-status 200

	g3bench h1 "${URL}" -x http://t1:toor@g3proxy.local:8080 --ok-status 200
	g3bench h1 "${URL}" -x http://t1:toor@g3proxy.local:8080 -p --ok-status 200

	g3bench h1 "${URL}" -x https://t1:toor@g3proxy.local:8443 --proxy-tls-ca-cert "${TEST_CA_CERT_FILE}" --ok-status 200
	g3bench h1 "${URL}" -x https://t1:toor@g3proxy.local:8443 --proxy-tls-ca-cert "${TEST_CA_CERT_FILE}" -p --ok-status 200

	g3bench h1 "${URL}" -x socks5h://t1:toor@g3proxy.local:1080 --ok-status 200
}

test_http_post()
{
	URL=$1

	g3bench h1 "${URL}" --method POST --payload 31323334 --ok-status 200
	g3bench h1 "${URL}" --method POST --payload 31323334 --binary --ok-status 200
	g3bench h1 "${URL}" --method POST --payload name=foo -H "Content-Type: application/x-www-form-urlencoded" --ok-status 200
}

test_http_get http://httpbin.local/get
test_http_get http://httpbin.local:2080/get

test_http_post http://httpbin.local/post
test_http_post http://httpbin.local:2080/post

# Https

test_https_get()
{
	URL=$1

	g3bench h1 "${URL}" --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"
	g3bench h1 "${URL}" -H "Accept: application/json" --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"

	g3bench h1 "${URL}" -x http://t1:toor@g3proxy.local:8080 --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"
	g3bench h1 "${URL}" -x http://t1:toor@g3proxy.local:8080 -p --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"

	g3bench h1 "${URL}" -x https://t1:toor@g3proxy.local:8443 --proxy-tls-ca-cert "${TEST_CA_CERT_FILE}" --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"
	g3bench h1 "${URL}" -x https://t1:toor@g3proxy.local:8443 --proxy-tls-ca-cert "${TEST_CA_CERT_FILE}" -p --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"

	g3bench h1 "${URL}" -x socks5h://t1:toor@g3proxy.local:1080 --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"
}

test_https_post()
{
	URL=$1

	g3bench h1 "${URL}" --method POST --payload 31323334 --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"
	g3bench h1 "${URL}" --method POST --payload 31323334 --binary --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"
	g3bench h1 "${URL}" --method POST --payload name=foo -H "Content-Type: application/x-www-form-urlencoded" --ok-status 200 --tls-ca-cert "${TEST_CA_CERT_FILE}"
}

test_https_get https://httpbin.local:9443/get
test_https_get https://httpbin.local:2443/get

test_https_post https://httpbin.local:9443/post
test_https_post https://httpbin.local:2443/post
