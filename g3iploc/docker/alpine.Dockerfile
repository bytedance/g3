FROM rust:alpine AS builder
WORKDIR /usr/src/g3
COPY . .
RUN apk add --no-cache musl-dev
ENV RUSTFLAGS="-Ctarget-feature=-crt-static"
RUN cargo build --profile release-lto -p g3iploc

FROM alpine:latest
RUN apk add --no-cache libgcc
COPY --from=builder /usr/src/g3/target/release-lto/g3iploc /usr/bin/g3iploc
ENTRYPOINT ["/usr/bin/g3iploc"]
CMD ["-Vvv"]
