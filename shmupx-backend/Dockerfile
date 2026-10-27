# Imagem do shmupx-backend para o Cloud Run. Nenhuma credencial entra na imagem: no Cloud Run o
# Firestore usa a conta de serviço do serviço, e o client secret do Google vem do Secret Manager.
FROM rust:1-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/shmupx-backend /usr/local/bin/shmupx-backend
ENV PORT=8080
EXPOSE 8080
CMD ["shmupx-backend"]
