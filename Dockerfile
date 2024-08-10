FROM rust:1.80.1-alpine3.20 AS builder

RUN apk update && apk add --no-cache musl-dev

WORKDIR /app

COPY Cargo.* ./

# Downloading and building dependencies (with an empty src/main.rs)
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release

# Compiling the actual binary
COPY src/ src
RUN touch -a -m src/main.rs
RUN cargo build --release

# Final image
FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=builder /app/target/release/gitlab-tokens-exporter .
EXPOSE 3000
ENTRYPOINT [ "./gitlab-tokens-exporter" ]
