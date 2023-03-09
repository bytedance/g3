#!/usr/bin/env python3

import argparse
import sys
import unittest

import requests
from requests.auth import HTTPBasicAuth


target_proxy = ''
target_site = 'http://httpbin.org'
server_ca_cert = None


class TestHttpBin(unittest.TestCase):
    def setUp(self):
        self.session = requests.Session()
        self.session.proxies.update({'http': target_proxy, 'https': target_proxy})
        self.session.headers.update({'accept': 'application/json'})
        self.session.verify = server_ca_cert

    def tearDown(self):
        self.session.close()

    def test_simple_get(self):
        r = self.session.get(f"{target_site}/get")
        self.assertEqual(r.status_code, 200)

    def test_basic_auth_get(self):
        r = self.session.get(f"{target_site}/basic-auth/name/pass")
        self.assertEqual(r.status_code, 401)

        r = self.session.get(f"{target_site}/basic-auth/name/pass", auth=HTTPBasicAuth('name', 'pass'))
        self.assertEqual(r.status_code, 200)

        r = self.session.get(f"{target_site}/basic-auth/name/pass", auth=HTTPBasicAuth('name', 'pas'))
        self.assertEqual(r.status_code, 401)

    def test_base64_decode(self):
        self.session.headers.update({'accept': 'text/html'})
        r = self.session.get(f"{target_site}/base64/SFRUUEJJTiBpcyBhd2Vzb21l")
        self.assertEqual(r.status_code, 200)
        self.assertEqual(r.text, "HTTPBIN is awesome")

    def test_post_continue(self):
        data = "Content to post"

        r = self.session.post(f"{target_site}/post", data=data)
        self.assertEqual(r.status_code, 200)

        r = self.session.post(f"{target_site}/post", data=data, headers={"Expect": "100-continue"})
        self.assertEqual(r.status_code, 200)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--proxy', '-x', nargs='?', help='Proxy URL')
    parser.add_argument('--site', '-T', nargs='?', help='Target Site', default=target_site)
    parser.add_argument('--ca-cert', nargs='?', help='CA Cert')

    (args, left_args) = parser.parse_known_args()

    if args.proxy is not None:
        target_proxy = args.proxy
    if args.ca_cert is not None:
        server_ca_cert = args.ca_cert
    target_site = args.site

    left_args.insert(0, sys.argv[0])

    unittest.main(argv=left_args)
