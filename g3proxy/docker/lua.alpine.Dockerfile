FROM nickblah/lua:5.4-luarocks-alpine AS lua
RUN apk add --no-cache build-base
# install lua modules here.
# dkjson is just an example.
# beware: lua's c modules (e.g. cjson),
# they need to be statically linked
# and won't work when installing in
# alpine, because alpine uses musl libc.
RUN luarocks install dkjson

FROM rust:alpine AS builder
WORKDIR /usr/src/g3
COPY . .
RUN apk add --no-cache musl-dev cmake capnproto-dev openssl-dev openssl-libs-static c-ares-dev c-ares-static lua5.4-dev pkgconfig
ENV PKG_CONFIG_PATH=/usr/lib/pkgconfig
RUN cargo build --profile release-lto \
 --no-default-features --features quic,c-ares,hickory,lua54 \
 -p g3proxy -p g3proxy-ctl

FROM alpine:latest
RUN apk add --no-cache ca-certificates
# Copy Lua binaries and modules from the lua stage
COPY --from=lua /usr/local/bin/lua /usr/local/bin/lua
COPY --from=lua /usr/local/bin/luarocks /usr/local/bin/luarocks
COPY --from=lua /usr/local/lib/lua /usr/local/lib/lua
COPY --from=lua /usr/local/share/lua /usr/local/share/lua
COPY --from=lua /usr/local/lib/luarocks /usr/local/lib/luarocks
# Copy the compiled binaries from the builder stage
COPY --from=builder /usr/src/g3/target/release-lto/g3proxy /usr/bin/g3proxy
COPY --from=builder /usr/src/g3/target/release-lto/g3proxy-ctl /usr/bin/g3proxy-ctl
ENTRYPOINT ["/usr/bin/g3proxy"]
CMD ["-Vvv"]