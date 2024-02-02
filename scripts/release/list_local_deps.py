#!/usr/bin/env python3

import argparse

import toml


def get_direct_dependencies(name: str, ver):
    next_dependencies = set()

    for pkg in data['package']:
        if pkg['name'] == name:
            if ver is not None:
                if pkg['version'] != ver:
                    continue
            for dep in pkg.get('dependencies', []):
                if ' ' in dep:
                    v = str(dep).split(' ', 2)
                    next_dependencies.add((v[0], v[1]))
                else:
                    next_dependencies.add((dep, None))

    return next_dependencies


def find_recursive_dependencies(name: str, ver):
    next_dependencies = get_direct_dependencies(name, ver)
    for (name, ver) in next_dependencies:
        if (name, ver) in all_dependencies:
            continue
        all_dependencies.add((name, ver))
        find_recursive_dependencies(name, ver)


def check_and_print_local_dependency(name: str, ver):
    for pkg in data['package']:
        if pkg['name'] == name:
            if ver is not None:
                if pkg['version'] != ver:
                    continue
            if 'source' in pkg:
                continue
            print(name)


def find_matching_crates(name: str):
    crates = set()
    for pkg in data['package']:
        if pkg['name'].startswith(name):
            crates.add(pkg['name'])
    return crates


parser = argparse.ArgumentParser(description="list all local dependencies for a specific component")
parser.add_argument('--lock-file', nargs=1, required=True, help="input Cargo.lock file")
parser.add_argument('--component', nargs=1, required=True, help="name of the component to release")

args = parser.parse_args()

lock_file = args.lock_file[0]
data = toml.load(lock_file)

component = args.component[0]

all_dependencies = set()
for crate in find_matching_crates(component):
    find_recursive_dependencies(crate, None)

for (name, ver) in all_dependencies:
    check_and_print_local_dependency(name, ver)
