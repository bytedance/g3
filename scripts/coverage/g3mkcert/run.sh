
g3mkcert()
{
	../../../target/debug/g3mkcert "$@"
}

# Root CA

g3mkcert --root --common-name "G3 Test CA" --rsa 2048 --output-cert rootCA-rsa.crt --output-key rootCA-rsa.key
g3mkcert --root --common-name "G3 Test CA" --ec256 --output-cert rootCA-ec256.crt --output-key rootCA-ec256.key
g3mkcert --root --common-name "G3 Test CA" --sm2 --output-cert rootCA-sm2.crt --output-key rootCA-sm2.key
g3mkcert --root --common-name "G3 Test CA" --ed25519 --output-cert rootCA-ed25519.crt --output-key rootCA-ed25519.key

# Intermediate CA

g3mkcert --intermediate --common-name "G3 Intermediate CA" --rsa 2048 --output-cert intermediateCA-rsa.crt --output-key intermediateCA-rsa.key --ca-cert rootCA-rsa.crt --ca-key rootCA-rsa.key
g3mkcert --intermediate --common-name "G3 Intermediate CA" --ec384 --output-cert intermediateCA-ec384.crt --output-key intermediateCA-ec384.key --ca-cert rootCA-ec256.crt --ca-key rootCA-ec256.key
g3mkcert --intermediate --common-name "G3 Intermediate CA" --sm2 --output-cert intermediateCA-sm2.crt --output-key intermediateCA-sm2.key --ca-cert rootCA-rsa.crt --ca-key rootCA-rsa.key
g3mkcert --intermediate --common-name "G3 Intermediate CA" --ed25519 --output-cert intermediateCA-ed25519.crt --output-key intermediateCA-ed25519.key --ca-cert rootCA-ed25519.crt --ca-key rootCA-ed25519.key

# TLS Server Certificate

g3mkcert --tls-server --host "www.example.com" --host "*.example.net" --rsa 2048 --output-cert tls-server-rsa.crt --output-key tls-server-rsa.key --ca-cert rootCA-rsa.crt --ca-key rootCA-rsa.key
g3mkcert --tls-server --host "www.example.com" --host "*.example.net" --ec256 --output-cert tls-server-ec256.crt --output-key tls-server-ec256.key --ca-cert intermediateCA-rsa.crt --ca-key intermediateCA-rsa.key
g3mkcert --tls-server --host "www.example.com" --host "*.example.net" --sm2 --output-cert tls-server-sm2.crt --output-key tls-server-sm2.key --ca-cert intermediateCA-sm2.crt --ca-key intermediateCA-sm2.key
g3mkcert --tls-server --host "www.example.com" --host "*.example.net" --ed25519 --output-cert tls-server-ed25519.crt --output-key tls-server-ed25519.key --ca-cert rootCA-rsa.crt --ca-key rootCA-rsa.key

# TLS Client Certificate

g3mkcert --tls-client --host "www.example.com" --rsa 4096 --output-cert tls-client-rsa.crt --output-key tls-client-rsa.key --ca-cert intermediateCA-ec384.crt --ca-key intermediateCA-ec384.key
g3mkcert --tls-client --host "www.example.com" --ec256 --output-cert tls-client-ec256.crt --output-key tls-client-ec256.key --ca-cert rootCA-ec256.crt --ca-key rootCA-ec256.key
g3mkcert --tls-client --host "www.example.com" --sm2 --output-cert tls-client-sm2.crt --output-key tls-client-sm2.key --ca-cert intermediateCA-rsa.crt --ca-key intermediateCA-rsa.key
g3mkcert --tls-client --host "www.example.com" --ed25519 --output-cert tls-client-ed25519.crt --output-key tls-client-ed25519.key --ca-cert intermediateCA-ed25519.crt --ca-key intermediateCA-ed25519.key

# TLCP Server Sign Certificate

g3mkcert --tlcp-server-sign --host "www.example.com" --host "*.example.net" --rsa 3072 --output-cert tlcp-server-sign-rsa.crt --output-key tlcp-server-sign-rsa.key --ca-cert rootCA-rsa.crt --ca-key rootCA-rsa.key
g3mkcert --tlcp-server-sign --host "www.example.com" --host "*.example.net" --sm2 --output-cert tlcp-server-sign-sm2.crt --output-key tlcp-server-sign-sm2.key --ca-cert intermediateCA-sm2.crt --ca-key intermediateCA-sm2.key

# TLCP Server Enc Certificate

g3mkcert --tlcp-server-enc --host "www.example.com" --host "*.example.net" --rsa 2048 --output-cert tlcp-server-enc-rsa.crt --output-key tlcp-server-enc-rsa.key --ca-cert rootCA-rsa.crt --ca-key rootCA-rsa.key
g3mkcert --tlcp-server-enc --host "www.example.com" --host "*.example.net" --sm2 --output-cert tlcp-server-enc-sm2.crt --output-key tlcp-server-enc-sm2.key --ca-cert intermediateCA-sm2.crt --ca-key intermediateCA-sm2.key

# TLCP Client Sign Certificate

g3mkcert --tlcp-client-sign --host "www.example.com" --rsa 3072 --output-cert tlcp-client-sign-rsa.crt --output-key tlcp-client-sign-rsa.key --ca-cert rootCA-rsa.crt --ca-key rootCA-rsa.key
g3mkcert --tlcp-client-sign --host "www.example.com" --sm2 --output-cert tlcp-client-sign-sm2.crt --output-key tlcp-client-sign-sm2.key --ca-cert intermediateCA-sm2.crt --ca-key intermediateCA-sm2.key

# TLCP Client Enc Certificate

g3mkcert --tlcp-client-enc --host "www.example.com" --rsa 2048 --output-cert tlcp-client-enc-rsa.crt --output-key tlcp-client-enc-rsa.key --ca-cert rootCA-rsa.crt --ca-key rootCA-rsa.key
g3mkcert --tlcp-client-enc --host "www.example.com" --sm2 --output-cert tlcp-client-enc-sm2.crt --output-key tlcp-client-enc-sm2.key --ca-cert intermediateCA-sm2.crt --ca-key intermediateCA-sm2.key
