FROM rust:1.78.0-slim

RUN apt-get update && apt-get install -y clang llvm
RUN apt-get install -y build-essential

RUN rustup target add wasm32-unknown-unknown
RUN rustup component add clippy rustfmt rust-std

WORKDIR /app

COPY . .

CMD ["make", "test-assets"]
