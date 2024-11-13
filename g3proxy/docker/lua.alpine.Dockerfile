FROM nickblah/lua:5.4-luarocks-alpine AS lua
RUN apk add --no-cache build-base
# install lua modules here.
# (lua-cjson is just an example)
RUN luarocks install lua-cjson

FROM rust:alpine AS builder
WORKDIR /usr/src/g3
COPY . .
RUN apk add --no-cache musl-dev cmake capnproto-dev openssl-dev c-ares-dev lua5.4-dev pkgconfig
ENV PKG_CONFIG_PATH=/usr/lib/pkgconfig
ENV RUSTFLAGS="-Ctarget-feature=-crt-static"
RUN cargo build --profile release-lto \
 --no-default-features --features quic,c-ares,hickory,lua54 \
 -p g3proxy -p g3proxy-ctl -p g3proxy-lua

FROM alpine:latest
# install dynamic libs
RUN apk add --no-cache ca-certificates libgcc c-ares lua5.4-libs
# Copy Lua binaries and modules from the lua stage
COPY --from=lua /usr/local/bin/lua /usr/local/bin/lua
COPY --from=lua /usr/local/bin/luarocks /usr/local/bin/luarocks
COPY --from=lua /usr/local/lib/lua /usr/local/lib/lua
COPY --from=lua /usr/local/share/lua /usr/local/share/lua
COPY --from=lua /usr/local/lib/luarocks /usr/local/lib/luarocks
# Copy the compiled binaries from the builder stage
COPY --from=builder /usr/src/g3/target/release-lto/g3proxy /usr/bin/g3proxy
COPY --from=builder /usr/src/g3/target/release-lto/g3proxy-ctl /usr/bin/g3proxy-ctl
COPY --from=builder /usr/src/g3/target/release-lto/g3proxy-lua /usr/bin/g3proxy-lua
ENTRYPOINT ["/usr/bin/g3proxy"]
CMD ["-Vvv"]