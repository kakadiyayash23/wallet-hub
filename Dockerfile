# Builds both Rust binaries. Use --target to pick one at `docker build` time.
FROM rust:1.90-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY services ./services
COPY migrations ./migrations
RUN cargo build --release --workspace

FROM debian:bookworm-slim AS api
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/api /usr/local/bin/api
EXPOSE 8080
CMD ["api"]

FROM debian:bookworm-slim AS mock-sportsbook
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/mock-sportsbook /usr/local/bin/mock-sportsbook
EXPOSE 8090
CMD ["mock-sportsbook"]
