
# RSA

g3bench keyless openssl --key "${TEST_RSA_KEY_FILE}" --sign --digest-type sha256 --verify "4d4dfb668f8c6ddd0227c03907515c58779914098a1bf8c169faafdea4d1b91d"

g3bench keyless openssl --key "${TEST_RSA_KEY_FILE}" --encrypt --verify "abcdef"
TO_DECRYPT_DATA=$(g3bench keyless openssl --key "${TEST_RSA_KEY_FILE}" --encrypt "abcdef" --no-summary --dump-result)
g3bench keyless openssl --key "${TEST_RSA_KEY_FILE}" --decrypt --verify --verify-data "abcdef" "${TO_DECRYPT_DATA}"

g3bench keyless openssl --key "${TEST_RSA_KEY_FILE}" --rsa-padding PKCS1 --rsa-private-encrypt --verify "abcdef"
TO_DECRYPT_DATA=$(g3bench keyless openssl --key "${TEST_RSA_KEY_FILE}" --rsa-padding PKCS1 --rsa-private-encrypt "abcdef" --no-summary --dump-result)
g3bench keyless openssl --key "${TEST_RSA_KEY_FILE}" --rsa-padding PKCS1 --rsa-public-decrypt --verify --verify-data "abcdef" "${TO_DECRYPT_DATA}"
g3bench keyless openssl --cert "${TEST_RSA_CERT_FILE}" --rsa-padding PKCS1 --rsa-public-decrypt --verify --verify-data "abcdef" "${TO_DECRYPT_DATA}"

# EC

g3bench keyless openssl --key "${TEST_EC_KEY_FILE}" --sign --digest-type sha256 --verify "4d4dfb668f8c6ddd0227c03907515c58779914098a1bf8c169faafdea4d1b91d"
