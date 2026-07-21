FROM --platform=$BUILDPLATFORM rust:1.97.0-slim-trixie@sha256:686a437ead83701e8f871e66e838c3ec55f46b5fc235b025756396ac823bdc51 AS builder

ARG TARGETARCH
ARG TARGETPLATFORM
ARG BUILDPLATFORM

RUN echo "I am running on $BUILDPLATFORM, building for $TARGETPLATFORM"

RUN apt update && \
    apt install -y --no-install-recommends wget xz-utils musl-dev && \
    rm -rf /var/cache/apt/lists && \
    rm -rf /var/cache/apt/archives

RUN case "$TARGETPLATFORM" in \
        linux/amd64) echo "x86_64-unknown-linux-musl" > /tmp/rust_target && \
                     echo "x86_64" > /tmp/arch && \
                     echo "export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-unknown-linux-musl-gcc" > /tmp/cc_env && \
                     echo "export CC=x86_64-unknown-linux-musl-gcc" >> /tmp/cc_env ;; \
        linux/arm64) echo "aarch64-unknown-linux-musl" > /tmp/rust_target && \
                     echo "aarch64" > /tmp/arch && \
                     echo "export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-unknown-linux-musl-gcc" > /tmp/cc_env && \
                     echo "export CC=aarch64-unknown-linux-musl-gcc" >> /tmp/cc_env ;; \
        *) echo "Unsupported target arch: $TARGETPLATFORM" && exit 1 ;; \
    esac && \
    rustup target add "$(cat /tmp/rust_target)"

RUN if [ "$BUILDPLATFORM" = "$TARGETPLATFORM" ]; then \
        echo -n "" > /tmp/cc_env ; \
    elif [ "$BUILDPLATFORM" != "linux/amd64" ]; then \
            echo "Cross-compilation is only supported from linux/amd64 to linux/arm64" && \
            # cf https://github.com/cross-tools/musl-cross/issues/13#issuecomment-3437856448
            exit 1 ; \
    else \
        # Download a musl-targeting cross-compiler
        wget -q "https://github.com/cross-tools/musl-cross/releases/download/20260515/$(cat /tmp/arch)-unknown-linux-musl.tar.xz" && \
        echo "90282c463498dcdab9b96a464a0925d53f30c884b2d7b25e3998999416ae34b8 $(cat /tmp/arch)-unknown-linux-musl.tar.xz" | sha256sum --check --status && \
        mkdir -p /opt/x-tools && \
        tar xf "$(cat /tmp/arch)-unknown-linux-musl.tar.xz" -C /opt/x-tools && \
        echo "export PATH=/opt/x-tools/$(cat /tmp/arch)-unknown-linux-musl/bin:$PATH" >> /tmp/cc_env ; \
    fi

WORKDIR /app

COPY Cargo.* ./

# Downloading and building our dependencies (with an empty src/main.rs)
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN . /tmp/cc_env && cargo build --release --locked --target $(cat /tmp/rust_target)

COPY src ./src
RUN touch src/main.rs && \
    . /tmp/cc_env && \
    cargo build --release --locked --target $(cat /tmp/rust_target) && \
    cp target/$(cat /tmp/rust_target)/release/gitlab-tokens-exporter /tmp/gitlab-tokens-exporter

# This stage is used to get the correct files into the final image
FROM alpine:3.24.1@sha256:28bd5fe8b56d1bd048e5babf5b10710ebe0bae67db86916198a6eec434943f8b AS files

RUN apk update && \
    apk upgrade --no-cache && \
    apk add --no-cache ca-certificates

RUN update-ca-certificates

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid 10001 \
    nonroot

# Final image
FROM scratch

# Workaround: podman doesn't support '--chmod=ugo=rX' when using COPY
# so we create the necessary directories (with the correct access rights) using WORKDIR
WORKDIR /etc/ssl/certs/
WORKDIR /

# /etc/nsswitch.conf may be used by some DNS resolvers
COPY --from=files --chmod=444 \
    /etc/passwd \
    /etc/group \
    /etc/nsswitch.conf \
    /etc/

COPY --from=files --chmod=444 /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

COPY --from=builder /tmp/gitlab-tokens-exporter /

USER nonroot:nonroot
ENTRYPOINT [ "/gitlab-tokens-exporter" ]
