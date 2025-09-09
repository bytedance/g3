# g3proxy Setup and Usage

## Overview
g3proxy is now successfully built and running on your machine! It's an enterprise-level forward proxy that supports HTTP, HTTPS, and SOCKS5 protocols.

## What's Running
- **HTTP Proxy**: `http://localhost:8080`
- **SOCKS5 Proxy**: `socks5://localhost:1080`

## Management Script
Use the `run-g3proxy.sh` script to manage the proxy:

```bash
# Check status
./run-g3proxy.sh status

# Stop the proxy
./run-g3proxy.sh stop

# Start the proxy
./run-g3proxy.sh start

# Restart the proxy
./run-g3proxy.sh restart

# Test configuration
./run-g3proxy.sh test
```

## Configuration Files
- `my-g3proxy.yaml` - Basic configuration (currently running)
- `advanced-g3proxy.yaml` - Advanced configuration with more features

## Testing the Proxy

### HTTP Proxy Test
```bash
# Test HTTP proxy
curl -x http://localhost:8080 https://httpbin.org/ip

# Test with verbose output
curl -v -x http://localhost:8080 https://httpbin.org/ip
```

### SOCKS5 Proxy Test
```bash
# Test SOCKS5 proxy
curl --socks5 localhost:1080 https://httpbin.org/ip

# Test with authentication (if configured)
curl --socks5 user:pass@localhost:1080 https://httpbin.org/ip
```

### Browser Configuration
Configure your browser to use:
- **HTTP Proxy**: `localhost:8080`
- **SOCKS5 Proxy**: `localhost:1080`

## Features Available
- HTTP/HTTPS proxy support
- SOCKS5 proxy with UDP support
- DNS resolution with multiple servers
- Happy Eyeballs algorithm for IPv4/IPv6
- Connection pooling and reuse
- Idle connection management
- Metrics collection (StatsD)
- Hot configuration reload
- Multi-threaded processing

## Advanced Features (in advanced-g3proxy.yaml)
- TLS termination and encryption
- Speed limiting
- Multiple server types
- Enhanced logging
- Metrics collection

## Logs and Monitoring
- Check process status: `ps aux | grep g3proxy`
- Check listening ports: `lsof -i :8080 -i :1080`
- View logs: The proxy logs to stdout/stderr

## Stopping the Proxy
```bash
# Using the management script
./run-g3proxy.sh stop

# Or manually
kill $(cat /tmp/g3proxy.pid)
```

## Configuration Examples
The proxy supports many advanced features including:
- User authentication
- Access control lists (ACLs)
- ICAP adaptation
- TLS interception
- Protocol inspection
- Load balancing
- Failover
- GeoIP routing

Check the `examples/` directory for more configuration examples.

## Next Steps
1. Test the proxy with your applications
2. Configure authentication if needed
3. Set up monitoring and logging
4. Explore advanced features in the documentation
5. Customize the configuration for your needs

The proxy is now ready for use! 🚀
