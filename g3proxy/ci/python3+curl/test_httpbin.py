#!/usr/bin/env python3

import argparse
import sys
import unittest
import base64
from io import BytesIO
from urllib.parse import urlencode

import pycurl

target_site = 'http://httpbin.org'
target_ca_cert = None
target_proxy = None
proxy_ca_cert = None
local_resolve = None
request_target_prefix = None
no_auth = False

ACCEPT_JSON = 'Accept: application/json'
ACCEPT_HTML = 'Accept: text/html'


class TestHttpBin(unittest.TestCase):
    def setUp(self):
        self.buffer = BytesIO()

        self.c = pycurl.Curl()
        self.c.setopt(pycurl.HTTP_VERSION, pycurl.CURL_HTTP_VERSION_1_1)
        self.c.setopt(pycurl.WRITEFUNCTION, self.buffer.write)
        self.c.setopt(pycurl.HTTPHEADER, [ACCEPT_JSON])
        if target_ca_cert is not None:
            self.c.setopt(pycurl.CAINFO, target_ca_cert)
        if target_proxy is not None:
            self.c.setopt(pycurl.PROXY, target_proxy)
            if proxy_ca_cert is not None:
                self.c.setopt(pycurl.PROXY_CAINFO, proxy_ca_cert)
        if local_resolve is not None:
            self.c.setopt(pycurl.RESOLVE, [local_resolve])

    def tearDown(self):
        self.c.close()

    def set_url_and_request_target(self, path: str):
        self.c.setopt(pycurl.URL, f"{target_site}{path}")
        if request_target_prefix is not None:
            self.c.setopt(pycurl.REQUEST_TARGET, f"{request_target_prefix}{path}")

    def test_simple_get(self):
        self.set_url_and_request_target('/get')
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 200)

    def test_basic_auth_get(self):
        self.set_url_and_request_target('/basic-auth/name/pass')
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 401)

        if not no_auth:
            auth_header = "Authorization: Basic {}".format(base64.standard_b64encode(b'name:pass').decode('utf-8'))
            self.c.setopt(pycurl.HTTPHEADER, [ACCEPT_JSON, auth_header])
            self.c.perform()
            self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 200)

            auth_header = "Authorization: Basic {}".format(base64.standard_b64encode(b'name:pas').decode('utf-8'))
            self.c.setopt(pycurl.HTTPHEADER, [ACCEPT_JSON, auth_header])
            self.c.perform()
            self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 401)

    def test_base64_decode(self):
        self.set_url_and_request_target('/base64/SFRUUEJJTiBpcyBhd2Vzb21l')
        self.c.setopt(pycurl.HTTPHEADER, [ACCEPT_HTML])
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 200)
        self.assertEqual(self.buffer.getvalue(), b"HTTPBIN is awesome")

    def test_post_small(self):
        data = "Content to post"

        self.set_url_and_request_target('/post')
        self.c.setopt(pycurl.POSTFIELDS, data)
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 200)

        # add Expect and try again
        self.c.setopt(pycurl.HTTPHEADER, ['Expect: 100-continue'])
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 200)

    def test_post_large(self):
        post_data = {'data': "Content to post" * 1024 * 100}
        post_fields = urlencode(post_data)

        self.set_url_and_request_target('/post')
        self.c.setopt(pycurl.POSTFIELDS, post_fields)
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 200)

        # disable Expect and try again
        self.c.setopt(pycurl.HTTPHEADER, ['Expect:'])
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 200)

    def test_put_file(self):
        self.set_url_and_request_target('/put')
        self.c.setopt(pycurl.UPLOAD, 1)
        file = open(__file__)
        self.c.setopt(pycurl.READDATA, file)
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 200)
        file.close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--proxy', '-x', nargs='?', help='Proxy URL')
    parser.add_argument('--site', '-T', nargs='?', help='Target Site', default=target_site)
    parser.add_argument('--ca-cert', nargs='?', help='CA Cert')
    parser.add_argument('--proxy-ca-cert', nargs='?', help='Proxy CA Cert')
    parser.add_argument('--resolve', nargs='?', help='Local Resolve Record for curl')
    parser.add_argument('--request-target-prefix', nargs='?', help='Set request target')
    parser.add_argument('--no-auth', action='store_true', help='No http auth tests')

    (args, left_args) = parser.parse_known_args()

    if args.ca_cert is not None:
        target_ca_cert = args.ca_cert
    if args.proxy is not None:
        target_proxy = args.proxy
    if args.proxy_ca_cert is not None:
        proxy_ca_cert = args.proxy_ca_cert
    if args.resolve is not None:
        local_resolve = args.resolve
    if args.request_target_prefix is not None:
        request_target_prefix = args.request_target_prefix
    target_site = args.site
    no_auth = args.no_auth

    left_args.insert(0, sys.argv[0])

    unittest.main(argv=left_args)
