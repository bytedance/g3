#!/bin/sh

for user in "t1:dogood" "t2:dogood"
do

	HTTP_PROXY="http://${user}@127.0.0.1:8080"
	test_http_proxy_http_forward

	SOCKS5_PROXY="socks5h://${user}@127.0.0.1:1080"
	test_socks5_proxy_http
done
