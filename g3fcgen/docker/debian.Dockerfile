FROM rust:bookworm AS builder
WORKDIR /usr/src/g3
COPY . .
RUN cargo build --profile release-lto --features vendored-openssl -p g3fcgen

FROM debian:bookworm-slim
COPY --from=builder /usr/src/g3/target/release-lto/g3fcgen /usr/bin/g3fcgen
ENTRYPOINT ["/usr/bin/g3fcgen"]
CMD ["-Vvv"]
