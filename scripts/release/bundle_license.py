#!/usr/bin/env python3

import argparse
import json
import sys
from pathlib import Path

NO_TEXT_LICENSES = ["UNLICENSE"]
ALIAS_NAME_MAP = {
    "BSL-1.0": ["BOOST"],
    "MPL-2.0": ["MPL2"],
    "MIT-0": ["MIT0"],
}
NO_TEXT_CRATES = [
    "daemonize",  # in the root dir repo
]


def split_licenses(text: str):
    _licenses = []
    if "AND" in text:
        for v in text.split("AND"):
            _license = v.strip()
            if _license.startswith("("):
                _licenses += split_licenses(_license.strip("()"))
            else:
                _licenses.append(_license)
    elif "OR" in text:
        for v in text.split("OR"):
            _licenses.append(v.strip())
    elif "/" in text:
        for v in text.split("/"):
            _licenses.append(v.strip())
    else:
        _licenses.append(text)
    return _licenses


def find_license_file_with_ext(l: str, d: Path):
    path = d.joinpath(l)
    if path.exists():
        return path
    for ext in ["md", "txt"]:
        path = d.joinpath(f"{l}.{ext}")
        if path.exists():
            return path
    return None


def find_license_file(l: str, d: Path):
    values = []
    # replace space first, so Apache-2.0 WITH LLVM-exception will become Apache-2.0_WITH_LLVM-exception
    n = l.replace(" ", "_")
    # looking for the full name first
    values.append(n)
    # then omit the version
    if "-" in n:
        s = n.split("-", 1)
        values.append(s[0])
    # also look for special names
    mapped_values = []
    for v in values:
        if v in ALIAS_NAME_MAP:
            mapped_values += ALIAS_NAME_MAP[v]
    values += mapped_values
    for v in values:
        # lookup upper case
        upper = find_license_file_with_ext(f"LICENSE-{v.upper()}", d)
        if upper is not None:
            return upper
        # lookup lower case
        lower = find_license_file_with_ext(f"license-{v.lower()}", d)
        if lower is not None:
            return lower
    return None


def find_default_license_file(d: Path):
    return find_license_file_with_ext("LICENSE", d)


def print_license_not_found():
    print("Comment:")
    print(" no license content found in the crate source code,")
    print(" you should find them in the repository")


def print_dual_licenses(name: str, licenses, d: Path):
    # they may have already merged the license file, like https://github.com/BLAKE3-team/BLAKE3
    license_file = find_default_license_file(d)
    if license_file is not None:
        print_license_file(license_file)
        return

    for l in licenses:
        license_file = find_license_file(l, d)
        if license_file is None:
            if l.upper() in NO_TEXT_LICENSES:
                continue
            if raise_not_found:
                if name in NO_TEXT_CRATES:
                    print(f"\nLicense: {l}")
                    print_license_not_found()
                    continue
                raise Exception(f"no matching license file found for {name} license {l}")
            else:
                print(f"\nLicense: {l}")
                print_license_not_found()
                continue
        print(f"\nLicense: {l}")
        print_license_file(license_file)


def print_single_license(name: str, l: str, d: Path):
    license_file = find_license_file(l, d)
    if license_file is not None:
        print_license_file(license_file)
        return

    license_file = find_default_license_file(d)
    if license_file is not None:
        print_license_file(license_file)
        return

    if raise_not_found:
        if name in NO_TEXT_CRATES:
            print_license_not_found()
            return
        raise Exception("no license found for {name}")
    else:
        print_license_not_found()


def print_license_file(file: Path):
    with open(file, encoding='UTF-8') as f:
        for line in f.readlines():
            print(f" {line.rstrip()}")


parser = argparse.ArgumentParser(description="bundle license file for vendored crates")
parser.add_argument('--metadata', nargs=1, type=argparse.FileType('r'), default=sys.stdin, help="Cargo metadata")
parser.add_argument('--raise-not-found', '-r', action='store_true', default=False, help='')

args = parser.parse_args()
metadata = json.load(args.metadata)
packages = metadata['packages']
raise_not_found = args.raise_not_found

print("# This file contains the licenses that we bundled in the release source code tarball.")
print("#")
print("# The format is similar to debian/copyright file:")
print("#   https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/")
print("#")
print("# This file is auto generated. Please report to the upstream if possible:")
print("#   https://github.com/bytedance/g3")

for pkg in packages:
    p_name = pkg['name']
    p_version = pkg['version']
    p_repository = pkg.get('repository', None)
    if p_repository is None:
        continue
    print(f"\nCrate: {p_name}@{p_version}")
    print(f"Repository: {p_repository}")

    p_path = Path(pkg['manifest_path']).parent

    p_license = pkg.get('license', None)
    if p_license is None:
        p_license_file = pkg.get('license_file', None)
        if p_license_file is None:
            print("License: unknown")
        else:
            print("License:")
            print_license_file(p_path.joinpath(p_license_file))
    else:
        all_list = split_licenses(p_license)
        if len(all_list) == 1:
            print(f"License: {p_license}")
            print_single_license(p_name, all_list[0], p_path)
        else:
            print(f"License: {p_license}")
            print_dual_licenses(p_name, all_list, p_path)
