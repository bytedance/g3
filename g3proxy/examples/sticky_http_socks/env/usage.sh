chmod +x setup.sh teardown.sh
./setup.sh

# verify round-robin via proxy
/usr/bin/curl -v -x http://myservice.test:80 http://ipinfo.io
/usr/bin/curl -v -x http://myservice.test:80 http://ipinfo.io
/usr/bin/curl -v -x http://myservice.test:80 http://ipinfo.io

# verify SOCKS5 path if you have local SOCKS servers on 1081/1082/1083
/usr/bin/curl -v --socks5-hostname myservice.test:1080 http://ipinfo.io
/usr/bin/curl -v --socks5-hostname myservice.test:1080 http://ipinfo.io
/usr/bin/curl -v --socks5-hostname myservice.test:1080 http://ipinfo.io

# when done
./teardown.sh
