#!/usr/bin/env bash
set -e

# Create a directory for certificates
if [ -d certs ]; then
	rm -rf certs
fi
mkdir -p certs
cd certs

# Generate CA key and certificate
openssl genrsa -out ca.key 2048
openssl req -new -x509 -days 365 -key ca.key -subj "/CN=admission-webhook-ca" -out ca.crt

# Generate server key
openssl genrsa -out tls.key 2048

# Create a certificate signing request configuration
cat >csr.conf <<EOF
[req]
req_extensions = v3_req
distinguished_name = req_distinguished_name

[req_distinguished_name]

[v3_req]
basicConstraints = CA:FALSE
keyUsage = nonRepudiation, digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_names

[alt_names]
DNS.1 = localhost
IP.1 = 127.0.0.1
EOF

# Generate certificate signing request
openssl req -new -key tls.key -subj "/CN=localhost" -out server.csr -config csr.conf

# Sign the certificate
openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out tls.crt -days 365 -extensions v3_req -extfile csr.conf

# Clean up
rm server.csr csr.conf

echo "Certificates generated successfully in the certs directory"
