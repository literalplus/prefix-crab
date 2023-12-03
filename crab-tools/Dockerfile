FROM prefix-crab.local/builder AS builder

FROM debian:stable-slim AS runner

# Install shared lib needed by diesel / pg
RUN apt-get update && apt-get install -y libpq5 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/prefix-crab/target/release/crab-tools /usr/local/bin

ENTRYPOINT ["/usr/bin/env", "bash"]
