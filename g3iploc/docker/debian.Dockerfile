FROM rust:bookworm AS builder
WORKDIR /usr/src/g3
COPY . .
RUN cargo build --profile release-lto -p g3iploc

FROM debian:bookworm-slim
COPY --from=builder /usr/src/g3/target/release-lto/g3iploc /usr/bin/g3iploc
ENTRYPOINT ["/usr/bin/g3iploc"]
CMD ["-Vvv"]
