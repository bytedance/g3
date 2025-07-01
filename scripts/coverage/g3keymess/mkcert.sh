#!/bin/sh

set -e

SCRIPT_DIR=$(dirname $0)

cd "${SCRIPT_DIR}"

MKCERT="../../../target/debug/g3mkcert"

$MKCERT --root --common-name "g3 root" --output-cert rootCA.pem --output-key rootCA-key.pem

$MKCERT --tls-server --ca-cert rootCA.pem --ca-key rootCA-key.pem --host g3proxy.local --rsa 2048 --output-cert rsa2048.crt --output-key rsa2048.key
$MKCERT --tls-server --ca-cert rootCA.pem --ca-key rootCA-key.pem --host g3proxy.local --ec256 --output-cert ec256.crt --output-key ec256.key
$MKCERT --tls-server --ca-cert rootCA.pem --ca-key rootCA-key.pem --host g3proxy.local --ed25519 --output-cert ed25519.crt --output-key ed25519.key

mv *.key keys/
