# Build Stage
FROM rust:1.88-alpine AS builder

WORKDIR /app

# Install build dependencies
RUN apk add --no-cache musl-dev pkgconfig openssl-dev

# Copy all Cargo.toml files
COPY Cargo.toml .
COPY lighter-rust/poseidon-hash/Cargo.toml lighter-rust/poseidon-hash/
COPY lighter-rust/crypto/Cargo.toml lighter-rust/crypto/
COPY lighter-rust/signer/Cargo.toml lighter-rust/signer/
COPY lighter-rust/api-client/Cargo.toml lighter-rust/api-client/

# Copy source code
COPY src ./src
COPY lighter-rust/poseidon-hash/src ./lighter-rust/poseidon-hash/src
COPY lighter-rust/crypto/src ./lighter-rust/crypto/src
COPY lighter-rust/crypto/tests ./lighter-rust/crypto/tests
COPY lighter-rust/crypto/benches ./lighter-rust/crypto/benches
COPY lighter-rust/signer/src ./lighter-rust/signer/src
COPY lighter-rust/signer/examples ./lighter-rust/signer/examples
COPY lighter-rust/api-client/src ./lighter-rust/api-client/src
COPY lighter-rust/api-client/examples ./lighter-rust/api-client/examples

# Build release binary
RUN cargo build --release

# Runtime Stage
FROM alpine:3.19

WORKDIR /app

RUN apk --no-cache add ca-certificates

COPY --from=builder /app/target/release/bot .
COPY config.yaml .

CMD ["./bot"]
