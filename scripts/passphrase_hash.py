#!/usr/bin/env python3

import os
import hashlib
import argparse
import secrets
import binascii


def is_float(s: str):
    try:
        _value = float(s)
        return True
    except ValueError:
        return False


def print_token(salt_s, md5_s, sha1_s, json_format=False):
    print("-=-=-=- for server -=-=-=-")
    if json_format:
        print({'salt': salt_s, 'md5': md5_s, 'sha1': sha1_s})
    else:
        print('salt:', salt_s)
        print('md5:', md5_s)
        print('sha1:', sha1_s)


def generate_token(key, salt_b: bytes):
    buf = key.encode('utf-8') + salt_b

    m = hashlib.md5()
    m.update(buf)
    md5_b = m.digest()

    m = hashlib.sha1()
    m.update(buf)
    sha1_b = m.digest()

    salt_s = binascii.b2a_hex(salt_b).decode('utf-8')
    md5_s = binascii.b2a_hex(md5_b).decode('utf-8')
    sha1_s = binascii.b2a_hex(sha1_b).decode('utf-8')

    return salt_s, md5_s, sha1_s


def generate_password():
    """ generate random password with characters in base64 urlsafe range """

    token = secrets.token_urlsafe(10)
    print("-=-=-=- for user -=-=-=-")
    print("password:", token)
    print("")
    return token


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description='generate hashed passphrase for g3proxy')
    parser.add_argument('password', nargs='?', help='password')
    parser.add_argument('--salt', nargs='?', help='salt text')
    parser.add_argument('--json', action='store_true', default=False, help='use json format')

    args = parser.parse_args()

    password = args.password
    if password is None:
        password = generate_password()

    salt = os.urandom(8)
    if args.salt is not None:
        salt = binascii.a2b_hex(args.salt)

    while True:
        salt, md5, sha1 = generate_token(password, salt)
        if is_float(salt) or is_float(md5) or is_float(sha1):
            continue
        else:
            print_token(salt, md5, sha1, args.json)
            break
