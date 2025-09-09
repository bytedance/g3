#!/bin/bash

# g3proxy Management Script

CONFIG_FILE="my-g3proxy.yaml"
PID_FILE="/tmp/g3proxy.pid"
BINARY="./target/release/g3proxy"

case "$1" in
    start)
        if [ -f "$PID_FILE" ] && kill -0 $(cat "$PID_FILE") 2>/dev/null; then
            echo "g3proxy is already running (PID: $(cat $PID_FILE))"
            exit 1
        fi
        echo "Starting g3proxy..."
        $BINARY --config-file "$CONFIG_FILE" --daemon --pid-file "$PID_FILE"
        sleep 2
        if [ -f "$PID_FILE" ] && kill -0 $(cat "$PID_FILE") 2>/dev/null; then
            echo "g3proxy started successfully (PID: $(cat $PID_FILE))"
            echo "HTTP Proxy: http://localhost:8080"
            echo "SOCKS Proxy: socks5://localhost:1080"
        else
            echo "Failed to start g3proxy"
            exit 1
        fi
        ;;
    stop)
        if [ -f "$PID_FILE" ] && kill -0 $(cat "$PID_FILE") 2>/dev/null; then
            echo "Stopping g3proxy (PID: $(cat $PID_FILE))..."
            kill $(cat "$PID_FILE")
            rm -f "$PID_FILE"
            echo "g3proxy stopped"
        else
            echo "g3proxy is not running"
        fi
        ;;
    restart)
        $0 stop
        sleep 2
        $0 start
        ;;
    status)
        if [ -f "$PID_FILE" ] && kill -0 $(cat "$PID_FILE") 2>/dev/null; then
            echo "g3proxy is running (PID: $(cat $PID_FILE))"
            echo "Listening ports:"
            lsof -i :8080 -i :1080 2>/dev/null | grep g3proxy
        else
            echo "g3proxy is not running"
        fi
        ;;
    test)
        echo "Testing configuration..."
        $BINARY --config-file "$CONFIG_FILE" --test-config
        ;;
    *)
        echo "Usage: $0 {start|stop|restart|status|test}"
        exit 1
        ;;
esac
