package com.example.httpbin;

import java.io.File;

import org.apache.http.HttpHost;
import org.apache.http.auth.AuthScope;
import org.apache.http.auth.UsernamePasswordCredentials;
import org.apache.http.client.CredentialsProvider;
import org.apache.http.client.methods.CloseableHttpResponse;
import org.apache.http.client.methods.HttpPost;
import org.apache.http.entity.ContentType;
import org.apache.http.entity.FileEntity;
import org.apache.http.impl.client.BasicCredentialsProvider;
import org.apache.http.impl.client.CloseableHttpClient;
import org.apache.http.impl.client.HttpClients;
import org.apache.http.util.EntityUtils;

public class AuthNoCachePostFile {
    static String proxyHost = "127.0.0.1";
    static int proxyPort = 13128; // proxy port
    static String proxyUser = "root";
    static String proxyPassword = "toor";

    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            System.out.println("File path not given");
            System.exit(1);
        }

        HttpHost proxy = new HttpHost(proxyHost, proxyPort);
        CredentialsProvider credsProvider = new BasicCredentialsProvider();
        // set auth for proxy
        credsProvider.setCredentials(
                new AuthScope(proxyHost, proxyPort),
                new UsernamePasswordCredentials(proxyUser, proxyPassword));

        // set client level cred and proxy
        CloseableHttpClient httpclient = HttpClients.custom()
                .setDefaultCredentialsProvider(credsProvider)
                .setProxy(proxy)
                .build();

        try {
            HttpPost httppost = new HttpPost("http://httpbin.org/post");

            File file = new File(args[0]);

            FileEntity reqEntity = new FileEntity(file, ContentType.APPLICATION_OCTET_STREAM);
            reqEntity.setChunked(true);
            httppost.setEntity(reqEntity);

            System.out.println("Executing request: " + httppost.getRequestLine());
            // do not set any execution context with auth cache
            CloseableHttpResponse response = httpclient.execute(httppost);
            try {
                System.out.println("----------------------------------------");
                System.out.println(response.getStatusLine());
                System.out.println(EntityUtils.toString(response.getEntity()));
            } finally {
                response.close();
            }
        } finally {
            httpclient.close();
        }
    }
}
