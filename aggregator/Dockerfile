FROM prefix-crab.local/builder AS builder

FROM debian:stable-slim AS runner

RUN useradd -m app
WORKDIR /home/app

# Install shared lib needed by diesel / pg
# libcurl is needed for surf-client needed by opentelemetry
RUN apt-get update && apt-get install -y libpq5 libcurl4-openssl-dev && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/prefix-crab/target/release/aggregator /usr/local/bin

USER 1000

ENTRYPOINT ["/usr/local/bin/aggregator"]
