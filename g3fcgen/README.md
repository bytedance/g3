# g3fcgen

g3fcgen is a server certificate generator/issuer to be used with g3proxy to enable TLS interception.
The protocol is defined [here](https://g3-project.readthedocs.io/projects/g3proxy/en/latest/protocol/helper/cert_generator.html).

g3fcgen is designed to run with g3proxy on the same host.
It is recommended to write you own implementation if you need to:

 - serve a cluster of g3proxy instances
 - cache the generated server certificate for a much longer time
 - add custom certificate issue methods

## How to build

To build debug binaries:
```shell
cargo build -p g3fcgen
```

To build release binaries:
```shell
cargo build --profile release-lto -p g3fcgen
```

To support SM2 certificates, you need to use *Tongsuo* by adding `--features vendored-tongsuo`.

## How to run

### Example

See this [simple example](examples/simple).

You can run `cargo run --bin g3fcgen -- -c g3fcgen/examples/simple/ -G port2999 -vv` to start it.

### Set UDP listen address

The default UDP listen address is *127.0.0.1:2999*, which is also the same default connect address in g3proxy.

There are two ways to change the UDP listen port:

- Via command line options

  You can set **-G port<port>** or **--group-name port<port>** to change the UDP listen port,
  the final listen address will be **[::]:<port>**.

  You can set the systemd instance name to *port<port>*, so when you run `systemctl start g3fcgen@port<port>`, 
  it will listen to the correct port automatically.

- Via environment variables

  You can use the environment variable **UDP_LISTEN_ADDR** to change the UDP listen address.
  You can add this environment variable to `/etc/g3fcgen/<instance name>/env` file to use this with
  systemd managed g3fcgen service.

### Hot Restart

It is not possible to do hot restart gracefully without using two ports.

If g3fcgen is running at port 3000, and g3proxy is also using port 3000, the steps are:

1. start a new g3fcgen service at port 3001 with the new config
2. reload g3proxy to use g3fcgen port 3001
3. stop g3fcgen running at port 3000

## Command line options

Just run `g3fcgen -h` to see all supported command line options.
