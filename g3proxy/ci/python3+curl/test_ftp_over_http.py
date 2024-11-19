#!/usr/bin/env python3

import argparse
import sys
import unittest
from io import BytesIO

import pycurl

target_site = 'ftp://127.0.0.1'
target_proxy = None
proxy_ca_cert = None


class TestFtp(unittest.TestCase):
    def setUp(self):
        self.buffer = BytesIO()

        self.c = pycurl.Curl()
        self.c.setopt(pycurl.WRITEFUNCTION, self.buffer.write)
        if target_proxy is not None:
            self.c.setopt(pycurl.PROXY, target_proxy)
            if proxy_ca_cert is not None:
                self.c.setopt(pycurl.PROXY_CAINFO, proxy_ca_cert)

    def tearDown(self):
        self.c.close()

    def set_url_and_request_target(self, path: str):
        self.c.setopt(pycurl.URL, f"{target_site}{path}")

    def test_001_list_root(self):
        self.set_url_and_request_target('/')
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 200)

    def test_002_list_file(self):
        self.set_url_and_request_target('/uploaded_file')
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 200)
        self.assertEqual(self.buffer.getvalue(), b'')

    def test_003_upload_file(self):
        self.set_url_and_request_target('/uploaded_file')
        self.c.setopt(pycurl.UPLOAD, 1)
        file = open(__file__)
        self.c.setopt(pycurl.READDATA, file)
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 200)
        file.close()

    def test_004_download_file(self):
        self.set_url_and_request_target('/uploaded_file')
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 200)
        file = open(__file__)
        data = file.read()
        self.assertEqual(self.buffer.getvalue().decode('utf-8'), data)
        file.close()

    def test_005_delete_file(self):
        self.set_url_and_request_target('/uploaded_file')
        self.c.setopt(pycurl.CUSTOMREQUEST, 'DELETE')
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 200)

    def test_006_delete_file(self):
        self.set_url_and_request_target('/uploaded_file')
        self.c.setopt(pycurl.CUSTOMREQUEST, 'DELETE')
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 404)

    def test_007_list_root(self):
        self.set_url_and_request_target('/')
        self.c.perform()
        self.assertEqual(self.c.getinfo(pycurl.RESPONSE_CODE), 200)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--proxy', '-x', nargs='?', help='Proxy URL', required=True)
    parser.add_argument('--site', '-T', nargs='?', help='Target Site', default=target_site)
    parser.add_argument('--proxy-ca-cert', nargs='?', help='Proxy CA Cert')

    (args, left_args) = parser.parse_known_args()

    if args.proxy_ca_cert is not None:
        proxy_ca_cert = args.proxy_ca_cert
    target_site = args.site
    target_proxy = args.proxy

    left_args.insert(0, sys.argv[0])

    unittest.main(argv=left_args)
