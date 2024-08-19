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
COPY --from=builder /usr/src/g3/target/release-lto/g3proxy /usr/bin/g3proxy
COPY --from=builder /usr/src/g3/target/release-lto/g3proxy-ctl /usr/bin/g3proxy-ctl
ENTRYPOINT ["/usr/bin/g3proxy"]
CMD ["-Vvv"]
