easy-proxy Well-Known Resource Identifier
====

URI suffix: easy-proxy

Change controller: [The G3 Project](https://github.com/bytedance/g3)

Specification document(s): https://github.com/bytedance/g3/blob/master/doc/easy-proxy.md

# Purpose

The easy-proxy well-known resource identifier is a convenient way for clients to access target servers via forward
proxies. The clients don't need to be aware of the forward proxy itself, instead the users only need to update the
target server uri to the format defined in this document, and then the client can work without any changes. This is
useful when the clients don't support forward proxy, or the users have no control over the clients.

# Format

The uri format for easy-proxy should be in this form:

```text
/.well-known/easy-proxy/{scheme}/{target_host}/{target_port}/{original_path}{?original_query}
```

The definition for each part:

- scheme

  The scheme of the original visit URI.

  The supported values:

    - http

      The proxy will visit `http://{target_host}:{target_port}/{original_path}{?original_query}`.

    - https

      The proxy will visit `https://{target_host}:{target_port}/{original_path}{?original_query}`.

    - ftp

      The proxy will visit `ftp://{target_host}:{target_port}/{original_path}{?original_query}`.

- target_host

  The hostname of the target server. The value should be a valid DNS domain name, or an IP address.

- target_port

  The port of the target server.

- original_path

  The original path field in the target server URI.

- original_query

  The original query field in the target server URI.

# Client Configuration

The user need to change the URI to the format defined above when configure the client.

Here are some examples:

- Visit an HTTP API via an HTTP proxy

    ```shell
    curl -v http://www.example.net/get?foo=bar
    ```

  can be written as

    ```shell
    curl -v http://proxy.example.com/.well-known/easy-proxy/http/www.example.net/80/get?foo=bar -d "name=curl"
    ```

- POST data to an HTTPS API via an HTTPS proxy

    ```shell
    curl -v https://www.example.net/post?foo=bar
    ```

  can be written as

    ```shell
    curl -v https://proxy.example.com/.well-known/easy-proxy/https/www.example.net/80/post?foo=bar -d "name=curl"
    ```

- Download ftp files via an HTTPS proxy

    ```shell
    curl -v ftp://ftp.example.net/data
    ```

  can be written as

    ```shell
    curl -v https://proxy.example.com/.well-known/easy-proxy/ftp/ftp.example.net/21/data
    ```

# Proxy Requirement

The proxy should also support normal forward proxy requests.

When a request arrives, the proxy should check the request first. For normal http forward request, the proxy should
forward the request without detecting the URIs. The proxy should check the URI only if the request is for the proxy
itself, the common conditions includes:

- The method is not CONNECT (required)
- The URI part in the request is not absolute, or the host part in the absolute URI is the server name of the proxy
- The server name in the Host header matches the server name of the proxy

The proxy server then need to check if the uri is an easy-proxy well-known URI.
If it is easy-proxy URI, then the proxy server should

1. extract the scheme/target_host/target_port
2. change the URI from

    ```text
    /.well-known/easy-proxy/{scheme}/{target_host}/{target_port}/{original_path}{?original_query}
    ```

   to

    ```text
    /{original_path}{?original_query}
    ```

3. forward the request the same way as normal forward proxy requests

    - the method must not be changed
    - all `Proxy-*` headers and all the hop-by-hop headers should be handled locally
    - all end-to-end headers except all `Proxy-*` headers should be forwarded normally

4. When forward the response back to the client, the proxy may update all `Location` headers to the easy-proxy URI form.

# Limitations

The easy-proxy URI defined here doesn't support all features of normal forward proxy protocol.

Some of them are:

- No proxy authorization support in the URI itself

  Users need to add the Proxy-Authorization headers to the client by other ways.

- Can not change the base URL that is used in HTML contents, so this won't work with browsers in most cases
