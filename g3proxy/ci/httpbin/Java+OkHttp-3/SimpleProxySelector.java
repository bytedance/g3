package com.example.httpbin;

import java.util.List;
import java.util.ArrayList;
import java.io.IOException;
import java.net.URI;
import java.net.Proxy;
import java.net.ProxySelector;
import java.net.SocketAddress;
import java.net.InetSocketAddress;

class SimpleProxySelector extends ProxySelector {
    private Proxy proxy;

    public SimpleProxySelector() {
        super();
    }

    public void SetProxy(String host, int port) {
        InetSocketAddress sa = new InetSocketAddress(host, port);
        proxy = new Proxy(Proxy.Type.HTTP, sa);
    }

    public final List<Proxy> select(URI uri) {
        // users may add code to select proxy based on uri here
        System.out.println("select proxy");
        List<Proxy> proxyList = new ArrayList<>();
        proxyList.add(proxy);
        return proxyList;
    }

    public final void connectFailed(URI uri, SocketAddress sa, IOException ioe) {
        System.out.println("connect failed");
        // users may handle error here
    }
}
