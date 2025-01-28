FROM rust:1.84.0-slim-bookworm AS builder

RUN apt update && apt upgrade -y && apt install -y libssl-dev pkg-config

WORKDIR /app

COPY Cargo.* ./

# Downloading and building our dependencies (with an empty src/main.rs)
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release

# Compiling the actual binary
COPY src/ src
RUN touch -a -m src/main.rs
RUN cargo build --release

# Final image
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=builder /app/target/release/gitlab-tokens-exporter .
EXPOSE 3000
ENTRYPOINT [ "./gitlab-tokens-exporter" ]
