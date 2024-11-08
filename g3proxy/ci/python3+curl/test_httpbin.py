#!/usr/bin/env python3

import argparse
import sys
import unittest
import base64
from io import BytesIO

import pycurl

target_site = 'http://httpbin.org'
target_ca_cert = None
target_proxy = None
proxy_ca_cert = None
local_resolve = None

ACCEPT_JSON = 'Accept: application/json'
ACCEPT_HTML = 'Accept: text/html'

class TestHttpBin(unittest.TestCase):
    def setUp(self):
        self.c = pycurl.Curl()
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

    def test_simple_get(self):
        self.c.setopt(pycurl.URL, f"{target_site}/get")
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 200)

    def test_basic_auth_get(self):
        self.c.setopt(pycurl.URL, f"{target_site}/basic-auth/name/pass")
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 401)

        if target_proxy is not None:
            auth_header = "Authorization: Basic {}".format(base64.standard_b64encode(b'name:pass').decode('utf-8'))
            self.c.setopt(pycurl.HTTPHEADER, [ACCEPT_JSON, auth_header])
            self.c.perform()
            self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 200)

            auth_header = "Authorization: Basic {}".format(base64.standard_b64encode(b'name:pas').decode('utf-8'))
            self.c.setopt(pycurl.HTTPHEADER, [ACCEPT_JSON, auth_header])
            self.c.perform()
            self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 401)

    def test_base64_decode(self):
        buffer = BytesIO()

        self.c.setopt(pycurl.URL, f"{target_site}/base64/SFRUUEJJTiBpcyBhd2Vzb21l")
        self.c.setopt(pycurl.HTTPHEADER, [ACCEPT_HTML])
        self.c.setopt(pycurl.WRITEFUNCTION, buffer.write)
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 200)
        self.assertEqual(buffer.getvalue(), b"HTTPBIN is awesome")

    def test_post_continue(self):
        data = "Content to post"

        self.c.setopt(pycurl.URL, f"{target_site}/post")
        self.c.setopt(pycurl.POSTFIELDS, data)
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 200)

        self.c.setopt(pycurl.HTTPHEADER, ['Expect: 100-continue'])
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 200)

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--proxy', '-x', nargs='?', help='Proxy URL')
    parser.add_argument('--site', '-T', nargs='?', help='Target Site', default=target_site)
    parser.add_argument('--ca-cert', nargs='?', help='CA Cert')
    parser.add_argument('--proxy-ca-cert', nargs='?', help='Proxy CA Cert')
    parser.add_argument('--resolve', nargs='?', help='Local Resolve Record for curl')

    (args, left_args) = parser.parse_known_args()

    if args.ca_cert is not None:
        target_ca_cert = args.ca_cert
    if args.proxy is not None:
        target_proxy = args.proxy
    if args.proxy_ca_cert is not None:
        proxy_ca_cert = args.proxy_ca_cert
    if args.resolve is not None:
        local_resolve = args.resolve
    target_site = args.site

    left_args.insert(0, sys.argv[0])

    unittest.main(argv=left_args)
