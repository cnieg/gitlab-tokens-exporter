FROM rust:1.84.0-slim-bookworm AS builder

ARG TARGETARCH

RUN apt update && apt upgrade -y && apt install -y musl-dev musl-tools
RUN if [ "$TARGETARCH" = "amd64" ]; then \
		rustup target add x86_64-unknown-linux-musl ; \
	elif [ "$TARGETARCH" = "arm64" ]; then \
		rustup target add aarch64-unknown-linux-musl ; \
	else \
		echo "Unsupported architecture: $TARGETARCH" ; \
		exit 1 ; \
	fi

WORKDIR /app

COPY Cargo.* ./

# Downloading and building our dependencies (with an empty src/main.rs)
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN if [ "$TARGETARCH" = "amd64" ]; then \
		cargo build --release --target=x86_64-unknown-linux-musl ; \
	elif [ "$TARGETARCH" = "arm64" ]; then \
		cargo build --release --target=aarch64-unknown-linux-musl ; \
	fi

# Compiling the actual binary
COPY src/ src
RUN touch -a -m src/main.rs
RUN if [ "$TARGETARCH" = "amd64" ]; then \
		cargo build --release --target=x86_64-unknown-linux-musl && \
		mv /app/target/x86_64-unknown-linux-musl/release/gitlab-tokens-exporter /app/target/ ; \
	elif [ "$TARGETARCH" = "arm64" ]; then \
		cargo build --release --target=aarch64-unknown-linux-musl && \
		mv /app/target/aarch64-unknown-linux-musl/release/gitlab-tokens-exporter /app/target/ ; \
	fi

# Final image
FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=builder /app/target/gitlab-tokens-exporter .
EXPOSE 3000
ENTRYPOINT [ "./gitlab-tokens-exporter" ]
