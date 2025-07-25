# --- Estágio 1: Builder ---
# Usamos uma imagem oficial do Rust (versão slim) para compilar a aplicação.
# A versão específica garante que as compilações sejam reproduzíveis.
FROM rust:1.88-slim AS builder

# Instala dependências de compilação necessárias para algumas crates (como reqwest com openssl).
RUN apt-get update && apt-get install -y libssl-dev pkg-config

WORKDIR /usr/src/app

# Copia os arquivos de manifesto do projeto.
COPY Cargo.toml Cargo.lock ./

# --- Otimização de Cache do Docker ---
# Criamos um projeto "dummy" e compilamos apenas as dependências.
# A camada de dependências só será reconstruída se o Cargo.toml ou Cargo.lock mudarem.
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    RUSTFLAGS="--cfg tokio_unstable" cargo build --release --quiet

# Agora, copia o código-fonte real da sua aplicação.
COPY ./src ./src

# Remove o binário dummy para garantir uma compilação limpa do seu código.
# O Rust substitui hifens por underscores nos nomes de dependência.
RUN rm -f target/release/deps/rust_backend* 

# Compila o seu código-fonte. Esta etapa será muito mais rápida, pois as
# dependências já estão em cache.
RUN RUSTFLAGS="--cfg tokio_unstable" cargo build --release --quiet


# --- Estágio 2: Final ---
# Usamos uma imagem base mínima do Debian para a imagem final, garantindo segurança e tamanho reduzido.
FROM debian:bookworm-slim

# Instala apenas as dependências de runtime necessárias.
# Para casos que usam OpenSSL, a dependência `libssl` é necessária.
RUN apt-get update && apt-get install -y libssl3 && rm -rf /var/lib/apt/lists/*

# Copia APENAS o binário compilado do estágio de "builder".
# O nome do binário é "rust-backend", conforme definido no Cargo.toml.
COPY --from=builder /usr/src/app/target/release/rust-backend /usr/local/bin/rust-backend

# Expõe a porta que a aplicação vai usar (informativo para o Docker).
EXPOSE 9999

# Comando para iniciar a aplicação quando o contêiner for executado.
CMD ["rust-backend"]