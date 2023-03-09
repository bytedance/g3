#!/usr/bin/env python3

import argparse
import socket
import ipaddress
import datetime
from urllib.parse import urlparse


import socks
import dns.message
import dns.query


dns_server = '223.5.5.5'


def to_proxy_addr(s: str):
    try:
        ip = ipaddress.IPv6Address(s.strip("[]"))
        return "{}".format(ip), socket.AF_INET6
    except ValueError:
        if s.find(":") >= 0:
            a = s.rsplit(":", 1)
            ip_str = a[0]
            if ip_str.endswith("]"):
                return "{}".format(ipaddress.IPv6Address(ip_str.strip("[]"))), socket.AF_INET6
            else:
                return "{}".format(ipaddress.IPv4Address(ip_str)), socket.AF_INET
        else:
            return "{}".format(ipaddress.IPv4Address(s)), socket.AF_INET


def query_for(sock, domain, verbose=False):
    msg = dns.message.make_query(domain, dns.rdatatype.A, rdclass=dns.rdataclass.IN, payload=4096)
    msg.flags |= dns.flags.AD
    msg.flags |= dns.flags.RD
    dns.query.send_udp(sock, msg, (dns_server, 53))
    (rsp, _) = dns.query.receive_udp(sock, (dns_server, 53), expiration=datetime.datetime.now().timestamp() + 60)
    if verbose:
        print(rsp.to_text())


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Query domains A records through socks5 proxy")
    parser.add_argument("-x", "--proxy", nargs='?', required=True)
    parser.add_argument("-f", "--domain-file", type=open, nargs='?', help="file contain domains, one per line")
    parser.add_argument("-v", "--verbose", action='store_true')
    parser.add_argument("--dns-server", nargs='?', help="DNS server IP")
    parser.add_argument("domains", nargs='*')
    args = parser.parse_args()

    if args.dns_server is not None:
        dns_server = args.dns_server

    url = urlparse(args.proxy)

    (proxy_addr, proxy_family) = to_proxy_addr(url.hostname)

    s = socks.socksocket(family=proxy_family, type=socket.SOCK_DGRAM, proto=0)
    s.set_proxy(proxy_type=socks.SOCKS5, addr=proxy_addr, port=url.port,
                username=url.username, password=url.password)
    s.bind(("", 0))
    if args.domain_file is not None:
        while True:
            domain = args.domain_file.readline()
            if domain == "":
                break
            domain = domain.strip("\r\n")
            if args.verbose:
                print("== {} ==>".format(domain))
            query_for(s, domain, verbose=args.verbose)
    if args.domains is not None:
        for domain in args.domains:
            if args.verbose:
                print("== {} ==>".format(domain))
            query_for(s, domain, verbose=args.verbose)
