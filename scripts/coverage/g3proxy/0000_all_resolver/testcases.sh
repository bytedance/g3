#!/bin/sh

resolver_query_verify()
{
	resolver=$1
	domain=$2
	expect=$3

	result=$("${PROJECT_DIR}"/target/debug/g3proxy-ctl -G ${TEST_NAME} -p $PROXY_PID resolver ${resolver} query ${domain})
	if [ $expect != $result ]
	then
		echo "domain ${domain} resolved on resolver ${resolver} is ${result}, but ${expect} is expected"
		exit 1
	fi
}

##
for resolver in main cares1 cares2 hickory
do
	echo "==== Query directly on resolver ${resolver}"
	resolver_query_verify ${resolver} g3proxy.local 127.0.0.1
	resolver_query_verify ${resolver} httpbin.local 127.0.0.1
done
