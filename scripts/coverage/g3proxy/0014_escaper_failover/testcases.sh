#!/bin/sh


HTTP_PROXY="http://127.0.0.1:8080"
test_http_proxy_http_forward
test_http_proxy_ftp_over_http
test_http_proxy_https_connect
test_http_proxy_https_forward


for port in 1080 1081
do
	SOCKS5_PROXY="socks5h://127.0.0.1:${port}"
	test_socks5_proxy_http
	test_socks5_proxy_dns


	SOCKS4_PROXY="socks4a://127.0.0.1:${port}"
	test_socks4_proxy_http
done