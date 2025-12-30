FROM --platform=$BUILDPLATFORM rust:1.92.0-slim-trixie AS builder

ARG TARGETARCH
ARG TARGETPLATFORM
ARG BUILDPLATFORM

SHELL ["/bin/bash", "-c"]

RUN echo "I am running on $BUILDPLATFORM, building for $TARGETPLATFORM"

RUN apt update && \
    apt install -y --no-install-recommends wget xz-utils musl-dev && \
    rm -rf /var/cache/apt/lists && \
    rm -rf /var/cache/apt/archives

RUN case "$TARGETPLATFORM" in \
        linux/amd64) echo "x86_64-unknown-linux-musl" > /tmp/rust_target ; \
                     echo "x86_64" > /tmp/arch ; \
                     echo "export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-unknown-linux-musl-gcc" > /tmp/cc_env ; \
                     echo "export CC=x86_64-unknown-linux-musl-gcc" >> /tmp/cc_env ;; \
        linux/arm64) echo "aarch64-unknown-linux-musl" > /tmp/rust_target ; \
                     echo "aarch64" > /tmp/arch ; \
                     echo "export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-unknown-linux-musl-gcc" > /tmp/cc_env ; \
                     echo "export CC=aarch64-unknown-linux-musl-gcc" >> /tmp/cc_env ;; \
        *) echo "Unsupported target arch: $TARGETPLATFORM" && exit 1 ;; \
    esac && \
    rustup target add $(cat /tmp/rust_target)

RUN if [ $BUILDPLATFORM == $TARGETPLATFORM ]; then \
        echo -n "" > /tmp/cc_env ; \
    else \
        if [ $BUILDPLATFORM != "linux/amd64" ]; then \
            echo "Cross-compilation is only supported from linux/amd64 to linux/arm64" ; \
            # cf https://github.com/cross-tools/musl-cross/issues/13#issuecomment-3437856448
            exit 1 ; \
        fi ; \
        # Download a musl-targeting cross-compiler
        wget https://github.com/cross-tools/musl-cross/releases/download/20250929/$(cat /tmp/arch)-unknown-linux-musl.tar.xz ; \
        mkdir -p /opt/x-tools ; \
        tar xf $(cat /tmp/arch)-unknown-linux-musl.tar.xz -C /opt/x-tools ; \
        echo "export PATH=/opt/x-tools/$(cat /tmp/arch)-unknown-linux-musl/bin:$PATH" >> /tmp/cc_env ; \
    fi

WORKDIR /app

COPY Cargo.* ./

# Downloading and building our dependencies (with an empty src/main.rs)
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN source /tmp/cc_env && cargo build --release --locked --target $(cat /tmp/rust_target)

COPY src ./src
RUN touch src/main.rs && \
    source /tmp/cc_env && \
    cargo build --release --locked --target $(cat /tmp/rust_target) && \
    cp target/$(cat /tmp/rust_target)/release/gitlab-tokens-exporter /tmp/gitlab-tokens-exporter

# Final image
FROM gcr.io/distroless/static-debian13:nonroot
COPY --from=builder /tmp/gitlab-tokens-exporter .
EXPOSE 3000
ENTRYPOINT [ "./gitlab-tokens-exporter" ]
