FROM rust:1.93-bookworm AS builder

WORKDIR /build

# Download pdfium binaries
RUN curl -sL "https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-linux-x64.tgz" \
    -o /tmp/pdfium.tgz && \
    mkdir -p /pdfium && \
    tar xzf /tmp/pdfium.tgz -C /pdfium && \
    rm /tmp/pdfium.tgz

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && \
    cargo build --release && \
    rm -rf src target/release/.fingerprint/refextract-*

# Build the actual binary
COPY src/ src/
COPY kbs/ kbs/
RUN cargo build --release

# Runtime image
FROM debian:bookworm-slim

COPY --from=builder /pdfium/lib/libpdfium.so /usr/local/lib/
COPY --from=builder /build/target/release/refextract /usr/local/bin/

RUN ldconfig

ENV PDFIUM_LIB_PATH=/usr/local/lib/libpdfium.so

ENTRYPOINT ["refextract"]
