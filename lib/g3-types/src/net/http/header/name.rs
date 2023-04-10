/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::borrow::Borrow;
use std::ops::Deref;

#[derive(Clone)]
pub enum HttpOriginalHeaderName {
    Common(&'static str),
    Custom(String),
}

macro_rules! original_headers {
    (
        $(
            $(#[$docs:meta])*
            $camel_str:literal, $lower_str:literal;
        )+
    ) => {
        impl HttpOriginalHeaderName {
            fn from_str(name_str: &str) -> Self {
                match name_str {
                    $(
                        $camel_str => HttpOriginalHeaderName::Common($camel_str),
                        $lower_str => HttpOriginalHeaderName::Common($lower_str),
                    )+
                    _ => HttpOriginalHeaderName::Custom(name_str.to_string()),
                }
            }
        }
    };
}

original_headers!(
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-accept
    "Accept", "accept";
    // https://www.rfc-editor.org/rfc/rfc8942.html#name-the-accept-ch-response-head
    "Accept-CH", "accept-ch";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-accept-charset
    "Accept-Charset", "accept-charset";
    // https://www.rfc-editor.org/rfc/rfc7089.html#section-2.1.1
    "Accept-Datetime", "accept-datetime";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-accept-encoding
    "Accept-Encoding", "accept-encoding";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-accept-language
    "Accept-Language", "accept-language";
    // https://www.rfc-editor.org/rfc/rfc5789.html#section-3.1
    "Accept-Patch", "accept-patch";
    // https://www.w3.org/TR/ldp/#header-accept-post
    "Accept-Post", "accept-post";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-accept-ranges
    "Accept-Ranges", "accept-ranges";
    // https://www.w3.org/TR/2007/WD-access-control-20071126/#access-control0
    "Access-Control", "access-control";
    // https://fetch.spec.whatwg.org/#http-responses
    "Access-Control-Allow-Credentials", "access-control-allow-credentials";
    "Access-Control-Allow-Headers", "access-control-allow-headers";
    "Access-Control-Allow-Methods", "access-control-allow-methods";
    "Access-Control-Allow-Origin", "access-control-allow-origin";
    "Access-Control-Expose-Headers", "access-control-expose-headers";
    "Access-Control-Max-Age", "access-control-max-age";
    // https://fetch.spec.whatwg.org/#http-requests
    "Access-Control-Request-Headers", "access-control-request-headers";
    "Access-Control-Request-Method", "access-control-request-method";
    // https://www.rfc-editor.org/rfc/rfc9111.html#name-age
    "Age", "age";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-allow
    "Allow", "allow";
    // https://www.rfc-editor.org/rfc/rfc7639.html#section-2
    "ALPN", "alpn";
    // https://www.rfc-editor.org/rfc/rfc7838.html#section-3
    "Alt-Svc", "alt-svc";
    // https://www.rfc-editor.org/rfc/rfc7838.html#section-5
    "Alt-Used", "alt-used";
    // https://www.rfc-editor.org/rfc/rfc4437.html#section-12.2
    "Apply-To-Redirect-Ref", "apply-to-redirect-ref";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-authorization
    "Authorization", "authorization";
    // https://www.rfc-editor.org/rfc/rfc8053.html#section-4
    "Authentication-Control", "authentication-control";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-authentication-info
    "Authentication-Info", "authentication-info";
    // https://www.rfc-editor.org/rfc/rfc9111.html#name-cache-control
    "Cache-Control", "cache-control";
    // https://www.rfc-editor.org/rfc/rfc9211.html#name-the-cache-status-http-respo
    "Cache-Status", "cache-status";
    // https://www.rfc-editor.org/rfc/rfc9297.html#name-the-capsule-protocol-header
    "Capsule-Protocol", "capsule-protocol";
    // https://www.rfc-editor.org/rfc/rfc9213.html#name-targeted-cache-control-head
    "CDN-Cache-Control", "cdn-cache-control";
    // https://www.rfc-editor.org/rfc/rfc8586.html#section-2
    "CDN-Loop", "cdn-loop";
    // https://www.rfc-editor.org/rfc/rfc8739.html#name-cert-not-before-and-cert-no
    "Cert-Not-After", "cert-not-after";
    "Cert-Not-Before", "cert-not-before";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-connection
    "Connection", "connection";
    // https://www.rfc-editor.org/rfc/rfc6266.html
    "Content-Disposition", "content-disposition";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-content-encoding
    "Content-Encoding", "content-encoding";
    // https://www.w3.org/TR/NOTE-drp
    "Content-ID", "content-id";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-content-language
    "Content-Language", "content-language";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-content-length
    "Content-Length", "content-length";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-content-location
    "Content-Location", "content-location";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-content-range
    "Content-Range", "content-range";
    // https://www.w3.org/TR/CSP/#csp-header
    "Content-Security-Policy", "content-security-policy";
    // https://www.w3.org/TR/CSP/#cspro-header
    "Content-Security-Policy-Report-Only", "content-security-policy-report-only";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-content-type
    "Content-Type", "content-type";
    // https://www.rfc-editor.org/rfc/rfc6265.html#section-4.2
    "Cookie", "cookie";
    // https://fetch.spec.whatwg.org/#cross-origin-resource-policy-header
    "Cross-Origin-Resource-Policy", "cross-origin-resource-policy";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-date
    "Date", "date";
    // https://www.rfc-editor.org/rfc/rfc4918.html#section-10.1
    "DAV", "dav";
    // https://www.rfc-editor.org/rfc/rfc4918.html#section-10.2
    "Depth", "depth";
    // https://www.rfc-editor.org/rfc/rfc4918.html#section-10.3
    "Destination", "destination";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-etag
    "ETag", "etag";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-expect
    "Expect", "expect";
    // https://www.rfc-editor.org/rfc/rfc9111.html#name-expires
    "Expires", "expires";
    // https://www.rfc-editor.org/rfc/rfc7239.html#section-4
    "Forwarded", "forwarded";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-from
    "From", "from";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-host-and-authority
    "Host", "host";
    // https://www.rfc-editor.org/rfc/rfc4918.html#section-10.4
    "If", "if";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-if-match
    "If-Match", "if-match";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-if-modified-since
    "If-Modified-Since", " if-modified-since";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-if-none-match
    "If-None-Match", "if-none-match";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-if-range
    "If-Range", "if-range";
    // https://www.rfc-editor.org/rfc/rfc6638.html#section-8.3
    "If-Schedule-Tag-Match", "if-schedule-tag-match";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-if-unmodified-since
    "If-Unmodified-Since", "if-unmodified-since";
    // https://www.rfc-editor.org/rfc/rfc2068#section-19.7.1.1
    "Keep-Alive", "keep-alive";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-last-modified
    "Last-Modified", "last-modified";
    // https://www.rfc-editor.org/rfc/rfc8288.html#section-3
    "Link", "link";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-location
    "Location", "location";
    // https://www.rfc-editor.org/rfc/rfc4918.html#section-10.5
    "Lock-Token", "lock-token";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-max-forwards
    "Max-Forwards", "max-forwards";
    // https://www.w3.org/TR/2007/WD-access-control-20071126/#method-check
    "Method-Check", "method-check";
    // https://www.w3.org/TR/2007/WD-access-control-20071126/#method-check-expires
    "Method-Check-Expires", "method-check-expires";
    // https://www.rfc-editor.org/rfc/rfc6454.html#section-7
    "Origin", "origin";
    // https://www.rfc-editor.org/rfc/rfc4918.html#section-10.6
    "Overwrite", "overwrite";
    // https://html.spec.whatwg.org/multipage/links.html#the-ping-headers
    "Ping-From", "ping-from";
    "Ping-To", "ping-to";
    // https://www.rfc-editor.org/rfc/rfc3648.html#section-6.1
    "Position", "position";
    // https://www.rfc-editor.org/rfc/rfc9111.html#name-pragma
    "Pragma", "pragma";
    // https://www.rfc-editor.org/rfc/rfc7240.html#section-2
    "Prefer", "prefer";
    // https://www.rfc-editor.org/rfc/rfc7240.html#section-3
    "Preference-Applied", "preference-applied";
    // https://www.rfc-editor.org/rfc/rfc9218.html#name-the-priority-http-header-fi
    "Priority", "priority";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-proxy-authenticate
    "Proxy-Authenticate", "proxy-authenticate";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-proxy-authorization
    "Proxy-Authorization", "proxy-authorization";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-proxy-authentication-info
    "Proxy-Authentication-Info", "proxy-authentication-info";
    // https://www.rfc-editor.org/rfc/rfc9209.html#section-2
    "Proxy-Status", "proxy-status";
    // https://www.rfc-editor.org/rfc/rfc7469.html#section-2.1
    "Public-Key-Directives", "public-key-directives";
    // https://www.rfc-editor.org/rfc/rfc7469.html#section-2.1
    "Public-Key-Pins-Report-Only", "public-key-pins-report-only";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-range
    "Range", "range";
    // https://www.rfc-editor.org/rfc/rfc4437.html#section-12.1
    "Redirect-Ref", "redirect-ref";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-referer
    "Referer", "referer";
    // https://html.spec.whatwg.org/multipage/document-lifecycle.html#the-refresh-header
    "Refresh", "refresh";
    // https://docs.oasis-open.org/odata/repeatable-requests/v1.0/cs01/repeatable-requests-v1.0-cs01.html#sec_HeaderFields
    "Repeatability-Client-ID", "repeatability-client-id";
    "Repeatability-First-Sent", "repeatability-first-sent";
    "Repeatability-Request-ID", "repeatability-request-id";
    "Repeatability-Result", "repeatability-result";
    // https://www.rfc-editor.org/rfc/rfc8555.html#section-9.3
    "Replay-Nonce", "replay-nonce";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-retry-after
    "Retry-After", "retry-after";
    // https://www.rfc-editor.org/rfc/rfc6638.html#section-8.1
    "Schedule-Reply", "schedule-reply";
    // https://www.rfc-editor.org/rfc/rfc6638.html#section-8.2
    "Schedule-Tag", "schedule-tag";
    // https://www.rfc-editor.org/rfc/rfc6455.html#section-11.3.3
    "Sec-WebSocket-Accept", "sec-webSocket-accept";
    // https://www.rfc-editor.org/rfc/rfc6455.html#section-11.3.2
    "Sec-WebSocket-Extensions", "sec-websocket-extensions";
    // https://www.rfc-editor.org/rfc/rfc6455.html#section-11.3.1
    "Sec-WebSocket-Key", "sec-websocket-key";
    // https://www.rfc-editor.org/rfc/rfc6455.html#section-11.3.4
    "Sec-WebSocket-Protocol", "sec-webSocket-protocol";
    // https://www.rfc-editor.org/rfc/rfc6455.html#section-11.3.5
    "Sec-WebSocket-Version", "sec-webSocket-version";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-server
    "Server", "server";
    // https://www.w3.org/TR/server-timing/#the-server-timing-header-field
    "Server-Timing", "server-timing";
    // https://www.rfc-editor.org/rfc/rfc6265.html#section-4.1
    "Set-Cookie", "set-cookie";
    // https://fetch.spec.whatwg.org/#sec-purpose-header
    "Sec-Purpose", "sec-purpose";
    // https://www.w3.org/TR/2000/NOTE-SOAP-20000508/#_Toc478383528
    "SOAPAction", "soapaction";
    // https://www.rfc-editor.org/rfc/rfc6797.html#section-6.1
    "Strict-Transport-Security", "strict-transport-security";
    // https://www.rfc-editor.org/rfc/rfc8594.html#section-3
    "Sunset", "sunset";
    // https://www.w3.org/TR/edge-arch/
    "Surrogate-Capabilities", "surrogate-capabilities";
    "Surrogate-Control", "surrogate-control";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-te
    "TE", "te";
    // https://www.rfc-editor.org/rfc/rfc8030.html#section-5.4
    "Topic", "topic";
    // https://www.rfc-editor.org/rfc/rfc4918.html#section-10.7
    "Timeout", "timeout";
    // https://www.w3.org/TR/resource-timing-1/#timing-allow-origin
    "Timing-Allow-Origin", "timing-allow-origin";
    // https://www.rfc-editor.org/rfc/rfc9112#name-transfer-encoding
    "Transfer-Encoding", "transfer-encoding";
    // https://www.w3.org/TR/trace-context/#traceparent-header-field-values
    "Traceparent", "traceparent";
    // https://www.w3.org/TR/trace-context/#tracestate-header
    "Tracestate", "tracestate";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-trailer
    "Trailer", "trailer";
    // https://www.rfc-editor.org/rfc/rfc8030.html#section-5.2
    "TTL", "ttl";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-upgrade
    "Upgrade", "upgrade";
    // https://www.rfc-editor.org/rfc/rfc8030.html#section-5.3
    "Urgency", "urgency";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-user-agent
    "User-Agent", "user-agent";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-vary
    "Vary", "vary";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-via
    "Via", "via";
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-www-authenticate
    "WWW-Authenticate", "www-authenticate";
    // https://fetch.spec.whatwg.org/#x-content-type-options-header
    "X-Content-Type-Options", "x-content-type-options";
    // https://html.spec.whatwg.org/multipage/document-lifecycle.html#the-x-frame-options-header
    "X-Frame-Options", "x-frame-options";
);

impl HttpOriginalHeaderName {
    fn as_str(&self) -> &str {
        match self {
            HttpOriginalHeaderName::Common(s) => s,
            HttpOriginalHeaderName::Custom(s) => s.as_str(),
        }
    }
}

impl<'a> From<&'a str> for HttpOriginalHeaderName {
    fn from(value: &'a str) -> Self {
        HttpOriginalHeaderName::from_str(value)
    }
}

impl Borrow<str> for HttpOriginalHeaderName {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Deref for HttpOriginalHeaderName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}
