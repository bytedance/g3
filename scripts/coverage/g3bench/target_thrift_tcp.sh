
# Kitex Thrift

KITEX_REQUEST=0c00010b00010000000a6d7920726571756573740000

git clone https://github.com/cloudwego/kitex-examples.git --depth 1

cd kitex-examples/hello

go build
./hello &
KITEX_PID=$!

cd -

sleep 1

g3bench thrift tcp --target 127.0.0.1:8888 --check-message-length 22 --binary echo ${KITEX_REQUEST}

g3bench thrift tcp --target 127.0.0.1:8888 --check-message-length 22 --binary --framed echo ${KITEX_REQUEST}

g3bench thrift tcp --target 127.0.0.1:8888 --check-message-length 22 --binary --framed --kitex-ttheader echo ${KITEX_REQUEST}

g3bench thrift tcp --target 127.0.0.1:8888 --check-message-length 22 --binary --kitex-ttheader echo ${KITEX_REQUEST}

g3bench thrift tcp --target 127.0.0.1:8888 --check-message-length 22 --binary --kitex-ttheader --info-kv "a:b" echo ${KITEX_REQUEST}
g3bench thrift tcp --target 127.0.0.1:8888 --check-message-length 22 --binary --kitex-ttheader --acl-token-kv "a:b" echo ${KITEX_REQUEST}
g3bench thrift tcp --target 127.0.0.1:8888 --check-message-length 22 --binary --kitex-ttheader --info-int-kv "4:not-default" echo ${KITEX_REQUEST}

kill -INT $KITEX_PID
