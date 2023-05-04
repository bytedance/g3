#!/usr/bin/env python3

import argparse
import json
import os
import sys

import requests
import semver
import toml


skip_prerelease = True
verbose_level = 0

script_name = sys.argv[0]


def get_locked_pkg(name: str):
    for p in packages:
        if p['name'] == name:
            return p
    return None


def get_locked_version(name: str):
    p = get_locked_pkg(name)
    return p['version']


def get_registry_path(name: str):
    if len(name) == 1:
        return f"/1/{name}"
    if len(name) == 2:
        return f"/2/{name}"
    if len(name) == 3:
        return f"/3/{name[0]}/{name}"
    return f"/{name[0]}{name[1]}/{name[2]}{name[3]}/{name}"


def get_latest_version(request_session, name: str):
    p = get_locked_pkg(name)
    registry_index = "https://index.crates.io"
    if p['source'] == 'registry+https://github.com/rust-lang/crates.io-index':
        pass
    else:
        raise Exception("unsupported registry")
    latest_version = "0.0.0"
    index_url = f"{registry_index}{get_registry_path(name)}"
    if verbose_level > 1:
        print(f"   GET {index_url}")
    rsp = request_session.get(url=index_url, timeout=30)
    for line in rsp.text.splitlines():
        d = json.loads(line)
        if d.get('yanked', False):
            continue
        vers = d['vers']
        if skip_prerelease:
            ver = semver.parse(vers)
            if ver.get('prerelease') is not None:
                continue
        if semver.compare(vers, latest_version) > 0:
            latest_version = vers
    return latest_version


def check():
    local_package_names = []
    local_packages = []
    for pkg in packages:
        if 'source' not in pkg:
            local_package_names.append(pkg['name'])
            local_packages.append(pkg)

    request_session = requests.session()
    outdated_packages = {}
    for pkg in local_packages:
        pkg_name = pkg['name']
        dependencies = pkg.get('dependencies', [])
        if verbose_level > 0:
            print(f"== Checking dependencies for package {pkg_name}")
        for dep in dependencies:
            if dep in local_package_names:
                continue
            r = dep.rsplit(maxsplit=1)
            dep_name = r[0]
            if len(r) > 1:
                dep_version = r[1]
            else:
                dep_version = get_locked_version(dep_name)
            next_version = get_latest_version(request_session, dep_name)
            if semver.compare(next_version, dep_version) > 0:
                if verbose_level > 0:
                    print(f"   {dep_name} {dep_version} -> {next_version}")
                pkg_id = f"{dep_name}/{dep_version}"
                if pkg_id in outdated_packages:
                    outdated_packages[pkg_id]['pkgs'].append(pkg_name)
                else:
                    outdated_packages[pkg_id] = {
                        'name': dep_name,
                        'cur': dep_version,
                        'next': next_version,
                        'pkgs': [pkg_name]
                    }

    if verbose_level > 0:
        print()
    for pkg in outdated_packages.values():
        print(f"{pkg['name']}: {pkg['cur']} => {pkg['next']}")
        for p in pkg['pkgs']:
            print(f" - {p}")


parser = argparse.ArgumentParser(description="Check for outdated dependencies")
parser.add_argument('-v', '--verbose', action='count', help="Add verbose output")
parser.add_argument('-p', '--prerelease', action='store_true',
                    help="Check for pre-release versions")
parser.add_argument('cargo_lock', type=str, nargs='?', help="Cargo.lock file")
args = parser.parse_args()

if args.verbose is not None:
    verbose_level = args.verbose
if args.prerelease:
    skip_prerelease = False

if args.cargo_lock is not None:
    cargo_lock = args.cargo_lock
else:
    cargo_lock = f"{os.path.dirname(script_name)}/../../Cargo.lock"
if not os.path.isabs(cargo_lock):
    cargo_lock = os.path.realpath(f"{os.curdir}/{cargo_lock}")
if verbose_level > 0:
    print(f"Using cargo lock file {cargo_lock}")

data = toml.load(cargo_lock)
packages = data['package']
check()
