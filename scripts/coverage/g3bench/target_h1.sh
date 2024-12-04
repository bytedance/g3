
# Http

test_http()
{
	URL=$1

	g3bench h1 ${URL} --ok-status 200

	g3bench h1 ${URL} -x http://t1:toor@g3proxy.local:8080 --ok-status 200
	g3bench h1 ${URL} -x http://t1:toor@g3proxy.local:8080 -p --ok-status 200

	g3bench h1 ${URL} -x https://t1:toor@g3proxy.local:8443 --proxy-tls-ca-cert ${SSL_CERT_FILE} --ok-status 200
	g3bench h1 ${URL} -x https://t1:toor@g3proxy.local:8443 --proxy-tls-ca-cert ${SSL_CERT_FILE} -p --ok-status 200

	g3bench h1 ${URL} -x socks5h://t1:toor@g3proxy.local:1080 --ok-status 200
}

test_http http://httpbin.local/get
test_http http://httpbin.local:2080/get

# Https

test_https()
{
	URL=$1

	g3bench h1 ${URL} --ok-status 200 --tls-ca-cert ${SSL_CERT_FILE}

	g3bench h1 ${URL} -x http://t1:toor@g3proxy.local:8080 --ok-status 200 --tls-ca-cert ${SSL_CERT_FILE}
	g3bench h1 ${URL} -x http://t1:toor@g3proxy.local:8080 -p --ok-status 200 --tls-ca-cert ${SSL_CERT_FILE}

	g3bench h1 ${URL} -x https://t1:toor@g3proxy.local:8443 --proxy-tls-ca-cert ${SSL_CERT_FILE} --ok-status 200 --tls-ca-cert ${SSL_CERT_FILE}
	g3bench h1 ${URL} -x https://t1:toor@g3proxy.local:8443 --proxy-tls-ca-cert ${SSL_CERT_FILE} -p --ok-status 200 --tls-ca-cert ${SSL_CERT_FILE}

	g3bench h1 ${URL} -x socks5h://t1:toor@g3proxy.local:1080 --ok-status 200 --tls-ca-cert ${SSL_CERT_FILE}
}

test_https https://httpbin.local:9443/get
test_https https://httpbin.local:2443/get
