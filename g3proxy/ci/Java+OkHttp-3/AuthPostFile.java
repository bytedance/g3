package com.example.httpbin;

import java.io.File;

import okhttp3.MediaType;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.RequestBody;
import okhttp3.Response;

public class AuthPostFile {
    static String proxyHost = "127.0.0.1";
    static int proxyPort = 13128; // proxy port
    static String proxyUser = "root";
    static String proxyPassword = "toor";

    static MediaType MEDIA_TYPE_OCTET_STREAM
            = MediaType.get("application/octet-stream");

    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            System.out.println("File path not given");
            System.exit(1);
        }

        SimpleProxySelector proxyAddr = new SimpleProxySelector();
        proxyAddr.SetProxy(proxyHost, proxyPort);
        SimpleAuthenticator proxyAuth = new SimpleAuthenticator();
        proxyAuth.SetAuth(proxyUser, proxyPassword);

        OkHttpClient client = new OkHttpClient.Builder()
                .proxySelector(proxyAddr)
                .proxyAuthenticator(proxyAuth)
                .build();

        File file = new File(args[0]);

        Request request = new Request.Builder()
                .url("http://httpbin.org/post")
                .post(RequestBody.create(MEDIA_TYPE_OCTET_STREAM, file))
                .build();

        try (Response response = client.newCall(request).execute()) {
            System.out.println("----------------------------------------");
            System.out.println("Status Code: " + response.code());
            System.out.println(response.body().string());
        }
    }
}
