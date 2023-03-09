#!/bin/sh

set -e

FTP_TMP_ROOT=/tmp/vsftpd
FTP_USERNAME=ftpuser
FTP_PASSWORD=ftppass

docker stop ftp httpbin || :
docker rm ftp httpbin || :

docker run -p 127.0.0.1:80:80 --name httpbin -d kennethreitz/httpbin

[ ! -d "${FTP_TMP_ROOT}" ] || rm -rf "${FTP_TMP_ROOT}"
mkdir ${FTP_TMP_ROOT}
docker run -d -v ${FTP_TMP_ROOT}:/home/vsftpd \
                -p 127.0.0.1:20:20 \
                -p 127.0.0.1:21:21 \
                -p 127.0.0.1:47400-47470:47400-47470 \
                -e FTP_USER=${FTP_USERNAME} \
                -e FTP_PASS=${FTP_PASSWORD} \
                -e PASV_ADDRESS=127.0.0.1 \
                --name ftp \
                -d bogem/ftp
