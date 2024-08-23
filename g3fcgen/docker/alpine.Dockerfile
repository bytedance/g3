FROM rust:alpine AS builder
WORKDIR /usr/src/g3
COPY . .
RUN apk add --no-cache musl-dev openssl-dev
ENV RUSTFLAGS="-Ctarget-feature=-crt-static"
RUN cargo build --profile release-lto -p g3fcgen

FROM alpine:latest
RUN apk add --no-cache libgcc
COPY --from=builder /usr/src/g3/target/release-lto/g3fcgen /usr/bin/g3fcgen
ENTRYPOINT ["/usr/bin/g3fcgen"]
CMD ["-Vvv"]
