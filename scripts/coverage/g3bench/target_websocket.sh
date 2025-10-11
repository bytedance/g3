
# Websocket H1

git clone https://github.com/gorilla/websocket.git --depth 1

cd websocket/examples/echo

go build server.go
./server -addr localhost:7080 &
WEBSOCKET_PID=$!

cd -

sleep 1

g3bench websocket h1 ws://127.0.0.1:7080/echo --payload 1234512312 --check-message-length 10
g3bench websocket h1 ws://127.0.0.1:7080/echo --payload 1234512312 --binary --check-message-length 5

kill -INT $WEBSOCKET_PID
