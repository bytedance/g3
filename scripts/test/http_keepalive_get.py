#!/usr/bin/env python3

import argparse
from urllib.parse import urlparse


import requests


def do_get(s, url):
    r = s.get(url)
    print(r.text)


def get_proxy_key(url):
    url = urlparse(url, scheme='http')
    key = url.scheme + '://' + url.hostname
    return key


def main():
    parser = argparse.ArgumentParser(description='Do multiple GET using persistent connection')
    parser.add_argument('-x', '--proxy', nargs='?', required=True)
    parser.add_argument('-c', '--count', nargs='?', type=int, default=2)
    parser.add_argument('url', nargs=1)
    args = parser.parse_args()

    url = args.url[0]
    s = requests.Session()
    key = get_proxy_key(url)
    s.proxies = {key: args.proxy}

    for i in range(0, args.count):
        print('Count', i, '=>')
        do_get(s, url)


if __name__ == '__main__':
    main()
