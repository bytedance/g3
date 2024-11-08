#!/bin/sh

##
for resolver in main cares1 cares2 hickory
do
	echo "==== Query directly on resolver ${resolver}"
	"${PROJECT_DIR}"/target/debug/g3proxy-ctl -G ${TEST_NAME} -p $PROXY_PID resolver ${resolver} query g3proxy.local
	"${PROJECT_DIR}"/target/debug/g3proxy-ctl -G ${TEST_NAME} -p $PROXY_PID resolver ${resolver} query httpbin.local
done
