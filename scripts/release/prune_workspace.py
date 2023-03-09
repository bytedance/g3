#!/usr/bin/env python3

import argparse

import toml


parser = argparse.ArgumentParser(description="simplify project toml config file for release purpose")
parser.add_argument('--input', nargs=1, required=True, help="input toml file")
parser.add_argument('--output', nargs='?', help="output toml file")
parser.add_argument('--component', nargs=1, required=True, help="name of the component to release")
parser.add_argument('libs', nargs='*', help="the list of dependency libs")

args = parser.parse_args()

input_file = args.input[0]
data = toml.load(input_file)

members = set()

component = args.component[0]
all_members = set()

for m in data['workspace']['members']:
    all_members.add(m)
    if m.startswith(component):
        members.add(m)
for lib in args.libs:
    members.add("lib/{}".format(lib))

# print all crates that need to be deleted
for m in all_members.difference(members):
    print(m)

if args.output is not None:
    data["workspace"]["members"] = members
    # delete default-members
    data["workspace"].pop("default-members", None)
    with open(args.output, 'w') as f:
        toml.dump(data, f)
