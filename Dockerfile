# syntax=docker/dockerfile:1
FROM rust:1.95.0-bookworm@sha256:6258907abe69656e41cd992e0b705cdcfabcbbe3db374f92ed2d47121282d4a1 AS build
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates cmake clang libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock rust-toolchain.toml clippy.toml ./
COPY .cargo .cargo
COPY vendor vendor
COPY xtask xtask
COPY src src
ENV CARGO_BUILD_JOBS=2
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target \
    cargo build --locked --release --bin codex-api-gateway \
    && install -Dm755 target/release/codex-api-gateway /rootfs/usr/local/bin/codex-api-gateway \
    && ldd target/release/codex-api-gateway > /tmp/runtime-libraries \
    && ! grep -q 'not found' /tmp/runtime-libraries \
    && awk '/=> \/|^[[:space:]]*\// { for (i=1;i<=NF;i++) if ($i ~ /^\//) print $i }' /tmp/runtime-libraries \
       | xargs -r -I '{}' cp --parents -L '{}' /rootfs \
    && install -Dm644 /etc/ssl/certs/ca-certificates.crt /rootfs/etc/ssl/certs/ca-certificates.crt \
    && install -d -m1777 /rootfs/tmp \
    && install -d -o65532 -g65532 /rootfs/data \
    && printf 'gateway:x:65532:65532:Gateway:/data:/sbin/nologin\n' > /rootfs/etc/passwd \
    && printf 'gateway:x:65532:\n' > /rootfs/etc/group
COPY LICENSE NOTICE /rootfs/usr/share/licenses/codex-api/
RUN for package in libc6 libgcc-s1 libssl3 zlib1g ca-certificates; do \
      install -Dm644 "/usr/share/doc/$package/copyright" \
        "/rootfs/usr/share/licenses/$package/copyright"; \
    done

FROM scratch
COPY --from=build /rootfs /
LABEL org.opencontainers.image.source="https://github.com/reonokiy/codex-api" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.title="Codex API Gateway"
ENV HOME=/data CODEX_HOME=/data SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
USER 65532:65532
WORKDIR /data
EXPOSE 8080
STOPSIGNAL SIGINT
ENTRYPOINT ["/usr/local/bin/codex-api-gateway"]
CMD ["--listen", "0.0.0.0:8080"]
