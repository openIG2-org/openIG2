# The rendezvous and relay service (docs/RENDEZVOUS.md) as a container image. Build from the repository root:
#   docker build -f deploy/rendezvous/Dockerfile -t openig2-rendezvous .
FROM rust:1.99-slim-trixie AS build
WORKDIR /src
COPY engine/Cargo.toml engine/Cargo.lock ./
COPY engine/crates crates
COPY engine/vendor vendor
RUN cargo build --release --locked -p ig2-rendezvous

# glibc and libgcc only, no shell; runs as uid 65532.
FROM gcr.io/distroless/cc-debian13:nonroot
LABEL org.opencontainers.image.source=https://github.com/openIG2-org/openIG2       org.opencontainers.image.description="openIG2 rendezvous and relay service"       org.opencontainers.image.licenses=AGPL-3.0-only
COPY --from=build /src/target/release/ig2-rendezvous /usr/local/bin/ig2-rendezvous
EXPOSE 29172/udp
ENTRYPOINT ["/usr/local/bin/ig2-rendezvous"]
CMD ["--listen", "0.0.0.0:29172"]
