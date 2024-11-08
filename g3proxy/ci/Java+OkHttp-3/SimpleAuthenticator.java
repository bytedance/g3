package com.example.httpbin;

import java.io.IOException;

import okhttp3.Authenticator;
import okhttp3.Credentials;
import okhttp3.Challenge;
import okhttp3.Route;
import okhttp3.Response;
import okhttp3.Request;

class SimpleAuthenticator implements Authenticator {
    String proxyAuth;

    public void SetAuth(String username, String password) {
        proxyAuth = Credentials.basic(username, password);
    }

    public Request authenticate(Route route, Response response) throws IOException {
        if (response.request().header("Proxy-Authorization") != null) {
            return null; // Give up, we've already failed to authenticate.
        }

        // the username and password can be selected by
        for (Challenge challenge : response.challenges()) {
            // If this is preemptive auth, use a preemptive credential.
            if (challenge.scheme().equalsIgnoreCase("OkHttp-Preemptive")) {
                // only for CONNECT, before sending request
                return response.request().newBuilder()
                        .header("Proxy-Authorization", proxyAuth)
                        .build();
            } else if (challenge.scheme().equalsIgnoreCase("Basic")) {
                // after recv 407 for the first non-auth request
                // no way to use preemptive auth for http forward at least at version 3.13
                // users may add the Proxy-Authorization header to their requests directly
                return response.request().newBuilder()
                        .header("Proxy-Authorization", proxyAuth)
                        .build();
            }
        }

        return null; // no supported auth scheme
    }
}
