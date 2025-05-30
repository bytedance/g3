FROM rust:alpine AS builder
WORKDIR /usr/src/g3
COPY . .
RUN apk add --no-cache musl-dev cmake capnproto-dev openssl-dev c-ares-dev
ENV RUSTFLAGS="-Ctarget-feature=-crt-static"
RUN cargo build --profile release-lto \
 --no-default-features --features rustls-ring,quic,c-ares \
 -p g3proxy -p g3proxy-ctl

FROM alpine:latest
RUN apk add --no-cache libgcc c-ares
RUN apk add --no-cache ca-certificates
COPY --from=builder /usr/src/g3/target/release-lto/g3proxy /usr/bin/g3proxy
COPY --from=builder /usr/src/g3/target/release-lto/g3proxy-ctl /usr/bin/g3proxy-ctl
ENTRYPOINT ["/usr/bin/g3proxy"]
CMD ["-Vvv"]
