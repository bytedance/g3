#!/usr/bin/env python3

import argparse

import toml


parser = argparse.ArgumentParser(description="simplify project toml config file for release purpose")
parser.add_argument('--input', nargs=1, required=True, help="input toml file")
parser.add_argument('--output', nargs='?', help="output toml file")
parser.add_argument('patches', nargs='*', help="the list of useless patches")

args = parser.parse_args()

input_file = args.input[0]
data = toml.load(input_file)

all_patches = data['patch']['crates-io']

for patch in args.patches:
    (name, version) = patch.split('/')
    if all_patches[name] is not None:
        if all_patches[name]['version'] == version:
            del(all_patches[name])

if args.output is not None:
    data["patch"]["crates-io"] = all_patches
    with open(args.output, 'w') as f:
        toml.dump(data, f)
