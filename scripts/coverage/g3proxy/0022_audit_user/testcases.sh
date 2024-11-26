#!/bin/sh


for user in t1:toor t2:toor t3:toor
do
	HTTP_PROXY="http://${user}@127.0.0.1:8080"
	test_http_proxy_http_forward
	# test_http_proxy_ftp_over_http
	test_http_proxy_https_connect
	test_http_proxy_https_forward
	test_http_proxy_h2


	SOCKS5_PROXY="socks5h://${user}@127.0.0.1:1080"
	test_socks5_proxy_http
	# test_socks5_proxy_dns
done