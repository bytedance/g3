#!/bin/sh


g3proxy_ctl escaper float_passive publish '{"type":"http","addr":"127.0.0.1:7080"}'


HTTP_PROXY="http://127.0.0.1:8080"
test_http_proxy_http_forward


for port in 1080 1081
do
	SOCKS5_PROXY="socks5h://127.0.0.1:${port}"
	test_socks5_proxy_http


	SOCKS4_PROXY="socks4a://127.0.0.1:${port}"
	test_socks4_proxy_http
done


g3proxy_ctl escaper float_passive publish '{"type":"https","addr":"127.0.0.1:7443", "tls_name": "g3proxy.local"}'


HTTP_PROXY="http://127.0.0.1:8080"
test_http_proxy_http_forward


for port in 1080 1081
do
	SOCKS5_PROXY="socks5h://127.0.0.1:${port}"
	test_socks5_proxy_http


	SOCKS4_PROXY="socks4a://127.0.0.1:${port}"
	test_socks4_proxy_http
done

g3proxy_ctl escaper float_passive publish '{"type":"socks5","addr":"127.0.0.1:6080"}'


HTTP_PROXY="http://127.0.0.1:8080"
test_http_proxy_http_forward


for port in 1080 1081
do
	SOCKS5_PROXY="socks5h://127.0.0.1:${port}"
	test_socks5_proxy_http
	test_socks5_proxy_dns


	SOCKS4_PROXY="socks4a://127.0.0.1:${port}"
	test_socks4_proxy_http
done


g3proxy_ctl escaper float_passive publish '{"type":"socks5s","addr":"127.0.0.1:6443", "tls_name": "g3proxy.local"}'


HTTP_PROXY="http://127.0.0.1:8080"
test_http_proxy_http_forward


for port in 1080 1081
do
	SOCKS5_PROXY="socks5h://127.0.0.1:${port}"
	test_socks5_proxy_http
	test_socks5_proxy_dns


	SOCKS4_PROXY="socks4a://127.0.0.1:${port}"
	test_socks4_proxy_http
done

