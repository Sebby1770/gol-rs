# syntax=docker/dockerfile:1

# ---- build stage ----
FROM rust:1-slim AS build
WORKDIR /src
# Copy manifests first for layer caching, then sources.
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY web ./web
RUN cargo build --release && strip target/release/gol

# ---- runtime stage ----
# Distroless: no shell, no package manager — a tiny, hardened image.
FROM gcr.io/distroless/cc-debian12 AS runtime
COPY --from=build /src/target/release/gol /usr/local/bin/gol
EXPOSE 8080
# The server binds 0.0.0.0 so it is reachable from outside the container.
ENTRYPOINT ["/usr/local/bin/gol"]
CMD ["serve", "--addr", "0.0.0.0:8080", "--pattern", "gosper", "--width", "120", "--height", "60"]
