FROM nickblah/lua:5.4-luarocks-alpine3.22 AS lua
RUN apk add --no-cache build-base
RUN luarocks install lua-cjson

FROM rust:alpine3.22 AS builder
WORKDIR /usr/src/g3
COPY . .
RUN apk add --no-cache musl-dev cmake capnproto-dev openssl-dev c-ares-dev lua5.4-dev pkgconfig
ENV PKG_CONFIG_PATH=/usr/lib/pkgconfig
ENV RUSTFLAGS="-Ctarget-feature=-crt-static"
RUN cargo build --profile release-lto \
  --no-default-features --features rustls-ring,quic,c-ares,lua54 \
  -p g3proxy -p g3proxy-ctl -p g3proxy-lua

FROM alpine:3.22
RUN apk add --no-cache ca-certificates libgcc c-ares lua5.4-libs
COPY --from=lua /usr/local/bin/lua /usr/local/bin/
COPY --from=lua /usr/local/bin/luarocks /usr/local/bin/
COPY --from=lua /usr/local/lib/lua /usr/local/lib/
COPY --from=lua /usr/local/share/lua /usr/local/share/
COPY --from=lua /usr/local/lib/luarocks /usr/local/lib/luarocks
COPY --from=builder /usr/src/g3/target/release-lto/g3proxy /usr/bin/
COPY --from=builder /usr/src/g3/target/release-lto/g3proxy-ctl /usr/bin/
COPY --from=builder /usr/src/g3/target/release-lto/g3proxy-lua /usr/bin/
ENTRYPOINT ["/usr/bin/g3proxy"]
CMD ["-Vvv"]