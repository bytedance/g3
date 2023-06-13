#!/bin/sh

set -e

SCRIPT_DIR=$(dirname $0)

cd "${SCRIPT_DIR}"

MKCERT="../../../target/debug/g3mkcert"

$MKCERT --root --common-name "g3 root" --output-cert rootCA.pem --output-key rootCA-key.pem

$MKCERT --tls-server --ca-cert rootCA.pem --ca-key rootCA-key.pem --host g3proxy.local --output-cert g3proxy.local.pem --output-key g3proxy.local-key.pem
$MKCERT --tls-server --ca-cert rootCA.pem --ca-key rootCA-key.pem --host httpbin.local --output-cert httpbin.local.pem --output-key httpbin.local-key.pem
