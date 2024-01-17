FROM prefix-crab.local/builder AS builder

FROM debian:stable-slim AS runner

RUN useradd -m app
WORKDIR /home/app

# Install shared lib needed by diesel / pg
RUN apt-get update && apt-get install -y libpq5 git && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/prefix-crab/target/release/seed-guard /usr/local/bin

USER 1000

ENTRYPOINT ["/usr/local/bin/seed-guard"]
