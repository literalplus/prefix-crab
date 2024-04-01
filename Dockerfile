# Leveraging the pre-built Docker images with 
# cargo-chef and the Rust toolchain
FROM docker.io/lukemathwalker/cargo-chef:latest-rust-latest AS chef
WORKDIR /usr/src/prefix-crab

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder 
COPY --from=planner /usr/src/prefix-crab/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json --workspace

# Build base libraries
COPY Cargo.toml .
COPY Cargo.lock .
COPY src src
RUN cargo build --release

# Build remaining applications
COPY . .
RUN cargo build --release --workspace
