FROM rust:1.87.0 AS builder

WORKDIR /app

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    libpq-dev \
    libssl-dev pkg-config \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml ./

RUN mkdir src && \
    echo "fn main() {println!(\"Generating lockfile with verbose output...\");}" > src/main.rs && \
    cargo generate-lockfile -vv

COPY src ./src
COPY migrations ./migrations
### COPY .env ./.env
COPY .sqlx ./.sqlx

RUN SQLX_OFFLINE=true cargo build --release --locked
RUN cargo install sqlx-cli --version 0.7.4 --no-default-features --features rustls,postgres


FROM debian:bookworm-slim AS runtime

ARG APP_USER=appuser
RUN groupadd -r ${APP_USER} && useradd --no-create-home -r -g ${APP_USER} ${APP_USER}

RUN apt-get update && apt-get install -y --no-install-recommends libssl3 ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/niwatori ./
COPY --from=builder /app/migrations ./migrations
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx
### COPY --from=builder /app/.env ./.env

RUN chown -R ${APP_USER}:${APP_USER} . && \
    chmod +x ./niwatori

USER ${APP_USER}

EXPOSE 8000
CMD ["./niwatori"]