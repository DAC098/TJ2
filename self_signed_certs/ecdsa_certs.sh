openssl ecparam -out localhost.ecdsa.key -name secp384r1 -genkey
openssl req -x509 -config localhost.cnf -days 365 -out localhost.ecdsa.crt -key localhost.ecdsa.key -nodes
