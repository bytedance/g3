Java OkHttp Testcases
----

This directory contains the testcases written in Java, using
[OkHttp 3.x](https://square.github.io/okhttp/)
as the http client library.

### How to run

```shell
# compile
javac -cp /usr/share/java/okhttp.jar -d ./build *java
# compress to jar, so it can be copied anywhere
cd build
jar cvf httpbin.jar com
# run
java -cp /usr/share/java/okhttp.jar:httpbin.jar com.example.httpbin.<classname> <params>
```

### Testcases

#### AuthPostFile

Reading a file and POST it's content to `http://httpbin.org/post`.

**PreemptiveBasicAuthentication** is not enabled, so we can use this testcase to
test the untrusted read functionality of the http proxy server.
