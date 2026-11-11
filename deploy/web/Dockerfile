# The browser client (ADR 0047) as a web server image; the game files come from a folder mounted at run time
# (ADR 0081), never from the image. Build from the repository root:
#   docker build -f deploy/web/Dockerfile -t openig2-web .
FROM rust:1.99-slim-trixie AS build
RUN rustup target add wasm32-unknown-unknown
WORKDIR /src
COPY VERSION ./
COPY engine/Cargo.toml engine/Cargo.lock engine/
COPY engine/vendor engine/vendor
# wasm-bindgen-cli must match the wasm-bindgen crate in the lock file (tools/build_web.py checks the same).
RUN cargo install wasm-bindgen-cli --locked --version "$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print }' engine/Cargo.lock)"
COPY engine/crates engine/crates
# The release's version (docs/VERSIONING.md); without it the page shows VERSION with -dev.
ARG OPENIG2_VERSION
RUN cd engine && if [ -z "$OPENIG2_VERSION" ]; then unset OPENIG2_VERSION; fi && cargo build --release --locked --target wasm32-unknown-unknown -p ig2-client \
 && wasm-bindgen --target web --no-typescript --out-name ig2_client --out-dir /out/pkg \
      target/wasm32-unknown-unknown/release/ig2-client.wasm
COPY web/index.html web/main.js /out/

# nginx as uid 101, no root; it writes only to /tmp.
FROM nginxinc/nginx-unprivileged:1-alpine
LABEL org.opencontainers.image.source=https://github.com/openIG2-org/openIG2 \
      org.opencontainers.image.description="openIG2 in the browser" \
      org.opencontainers.image.licenses=AGPL-3.0-only
COPY --from=build /out /usr/share/nginx/html
COPY deploy/web/nginx.conf /etc/nginx/conf.d/default.conf
# The script is executable in git (100755); a plain COPY keeps that mode, also without BuildKit.
COPY deploy/web/game-index.sh /docker-entrypoint.d/40-game-index.sh
EXPOSE 8080
