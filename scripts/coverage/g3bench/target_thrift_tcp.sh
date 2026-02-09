
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
g3bench thrift tcp --target 127.0.0.1:8888 --check-message-length 22 --binary --kitex-ttheader --acl-token "abcdefg" echo ${KITEX_REQUEST}
g3bench thrift tcp --target 127.0.0.1:8888 --check-message-length 22 --binary --kitex-ttheader --info-int-kv "4:not-default" echo ${KITEX_REQUEST}

kill -INT $KITEX_PID

# Thrift go tutorial

git clone https://github.com/apache/thrift.git --depth 1

cd thrift/tutorial/go

thrift --gen go:thrift_import=github.com/apache/thrift/lib/go/thrift,package_prefix=github.com/apache/thrift/tutorial/go/gen-go/ ../shared.thrift
thrift --gen go:thrift_import=github.com/apache/thrift/lib/go/thrift,package_prefix=github.com/apache/thrift/tutorial/go/gen-go/ ../tutorial.thrift

go build -o go-tutorial ./src/*.go
cd -

## Binary Framed

./thrift/tutorial/go/go-tutorial -server -addr 127.0.0.1:9090 -P binary --framed &
THRIFT_GO_PID=$!

sleep 1

g3bench thrift tcp --target 127.0.0.1:9090 --check-message-length 1 --binary --framed ping 00
g3bench thrift tcp --target 127.0.0.1:9090 --check-message-length 8 --binary --framed add "080001000000010800020000000100"

kill -INT $THRIFT_GO_PID

## Binary

./thrift/tutorial/go/go-tutorial -server -addr 127.0.0.1:9091 -P binary --buffered &
THRIFT_GO_PID=$!

sleep 1

g3bench thrift tcp --target 127.0.0.1:9091 --check-message-length 1 --binary ping 00
g3bench thrift tcp --target 127.0.0.1:9091 --check-message-length 8 --binary add "080001000000010800020000000100"

kill -INT $THRIFT_GO_PID

## Compact Framed

./thrift/tutorial/go/go-tutorial -server -addr 127.0.0.1:9092 -P compact --framed &
THRIFT_GO_PID=$!

sleep 1

g3bench thrift tcp --target 127.0.0.1:9092 --check-message-length 1 --compact --framed ping 00
g3bench thrift tcp --target 127.0.0.1:9092 --check-message-length 4 --compact --framed add "1502150200"

kill -INT $THRIFT_GO_PID

## Compact

./thrift/tutorial/go/go-tutorial -server -addr 127.0.0.1:9093 -P compact --buffered &
THRIFT_GO_PID=$!

sleep 1

g3bench thrift tcp --target 127.0.0.1:9093 --check-message-length 1 --compact ping 00
g3bench thrift tcp --target 127.0.0.1:9093 --check-message-length 4 --compact add "1502150200"

kill -INT $THRIFT_GO_PID
