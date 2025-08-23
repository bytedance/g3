#!/bin/sh

##
for resolver in google-first local-first local
do
	echo "==== Query directly on resolver ${resolver}"
	resolver_query_verify ${resolver} g3proxy.local 127.0.0.1
	resolver_query_verify ${resolver} httpbin.local 127.0.0.1
done
