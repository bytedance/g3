#!/bin/sh


g3proxy_ctl user-group g2 publish-user ${TESTCASE_DIR}/group_2.json


for port in 8080 8081 8082 8083
do
	for user in "t1:toor" "t2:toor" "t3:toor"
	do
		HTTP_PROXY="http://${user}@127.0.0.1:${port}"
		test_http_proxy_http_forward
		test_http_proxy_ftp_over_http
	done
done
