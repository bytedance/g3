#!/usr/bin/env python3

import argparse
import pathlib
import sys
import gzip


def dump_maxmind_mmdb(file: pathlib.Path, output):
    try:
        import maxminddb
    except ImportError:
        sys.exit(1)

    db = maxminddb.open_database(file)
    node_count = db.metadata().node_count
    step_count = node_count / 10

    node_handled = 0
    step_handled = 0
    for r in db:
        network = r[0]
        info = r[1]

        node_handled += 1
        step_handled += 1
        if step_handled > step_count:
            step_handled = 0
            print(f"handled {node_handled} of {node_count}")

        if not info:
            continue
        country_code = info.get('Country').get('Code')
        output.write(f"{network},{country_code}\n")


def dump_ipinfo_mmdb(file: pathlib.Path, output):
    try:
        import maxminddb
    except ImportError:
        sys.exit(1)

    db = maxminddb.open_database(file)
    node_count = db.metadata().node_count
    step_count = node_count / 10

    node_handled = 0
    step_handled = 0
    for r in db:
        network = r[0]
        info = r[1]

        node_handled += 1
        step_handled += 1
        if step_handled > step_count:
            step_handled = 0
            print(f"handled {node_handled} of {node_count}")

        if not info:
            continue
        country_code = info.get('country_code')
        output.write(f"{network},{country_code}\n")


def dump_ipfire_db(file: pathlib.Path, output):
    try:
        import location
    except ImportError:
        sys.exit(1)

    db = location.Database(f"{file}")
    networks = db.search_networks()
    for net in networks:
        output.write(f"{net},{net.country_code}\n")


def main():
    parser = argparse.ArgumentParser(description="Dump GeoIP database files to the format used by g3iploc")
    parser.add_argument('-i', '--input', type=pathlib.Path, required=True, metavar='<input file>')
    parser.add_argument('-o', '--output', type=pathlib.Path, required=True, metavar='<output file>',
                        default='g3_geoip_country.gz')
    vendor_group = parser.add_mutually_exclusive_group(required=True)
    vendor_group.add_argument('--maxmind', action='store_true')
    vendor_group.add_argument('--ipinfo', action='store_true')
    vendor_group.add_argument('--ipfire', action='store_true')

    args = parser.parse_args()

    with gzip.open(args.output, 'wt') as f:
        input_file = args.input
        if args.maxmind:
            if input_file.suffix == '.mmdb':
                dump_maxmind_mmdb(input_file, f)
        elif args.ipinfo:
            if input_file.suffix == '.mmdb':
                dump_ipinfo_mmdb(input_file, f)
        elif args.ipfire:
            dump_ipfire_db(input_file, f)


if __name__ == '__main__':
    main()
