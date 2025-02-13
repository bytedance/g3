#!/usr/bin/env python3

import argparse


def get_physical_core(i: int):
    with open(f"/sys/devices/system/cpu/cpu{i}/topology/core_id", 'r') as f:
        core = f.read()
    return core.rstrip()


def cpu_set_to_list(s: str):
    # Convert CPU SET mask to CPU ID list
    cpus = []
    i = 0
    for c in reversed(s):
        n = int(c, 16)

        if n & 0b0001:
            cpus.append(str(i))
        i += 1
        if n & 0b0010:
            cpus.append(str(i))
        i += 1
        if n & 0b0100:
            cpus.append(str(i))
        i += 1
        if n & 0b1000:
            cpus.append(str(i))
        i += 1
    return cpus


def print_by_core(l: [int]):
    core_map = {}
    for cpu in l:
        core = get_physical_core(cpu)
        if core in core_map:
            core_map[core].append(str(cpu))
        else:
            core_map[core] = [str(cpu)]
    sorted_map = sorted(core_map)
    for core in sorted_map:
        cpus = ','.join(core_map[core])
        print(f"CORE:{core}: {cpus}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description='Split a CPU SET into parts on different cores')
    parser.add_argument('set', nargs=1, help='The input CPU SET')
    parser.add_argument('--by-core', action='store_true', help='List CPU ID group by CORE ID')

    args = parser.parse_args()

    cpu_list = cpu_set_to_list(args.set[0])
    print("LIST: {}".format(','.join(cpu_list)));
    if args.by_core:
        print_by_core(cpu_list)
