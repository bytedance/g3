Java Apache HttpComponents Testcases
----

This directory contains the testcases written in Java, using
[Apache HttpComponents HttpClient 4.5](https://hc.apache.org/httpcomponents-client-4.5.x/index.html)
as the http client library.

### How to run

```shell
java -classpath /usr/share/java/httpclient.jar <filename>.jar
```

### Testcases

#### AuthNoCachePostFile

Reading a file and POST it's content to `http://httpbin.org/post`.

**PreemptiveBasicAuthentication** is not enabled, so we can use this testcase to
test the untrusted read functionality of the http proxy server.
