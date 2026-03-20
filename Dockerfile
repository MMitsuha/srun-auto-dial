FROM rust:bookworm AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/

RUN cargo build --release

# ---- Runtime ----
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/srun-auto-dial /usr/local/bin/srun-auto-dial
COPY srun.toml.example /etc/srun-auto-dial/srun.toml.example

EXPOSE 3000

ENTRYPOINT ["srun-auto-dial"]
CMD ["server"]
