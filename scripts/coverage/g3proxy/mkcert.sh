#!/bin/sh

set -e

SCRIPT_DIR=$(dirname $0)

cd "${SCRIPT_DIR}"

export CAROOT="."

mkcert g3proxy.local
mkcert httpbin.local
