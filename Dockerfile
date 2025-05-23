FROM --platform=${BUILDPLATFORM} ghcr.io/rust-cross/rust-musl-cross:aarch64-musl AS build-arm64
ENV BUILDTARGET="aarch64-unknown-linux-musl"

FROM --platform=${BUILDPLATFORM} ghcr.io/rust-cross/rust-musl-cross:x86_64-musl AS build-amd64
ENV BUILDTARGET="x86_64-unknown-linux-musl"

FROM build-${TARGETARCH} AS build
WORKDIR /src

COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --target=$BUILDTARGET --locked

COPY src src/

RUN touch src/main.rs && cargo build --release --target $BUILDTARGET
RUN mkdir /out/ && cp /src/target/$BUILDTARGET/release/patternify-bot /out/patternify-bot

FROM scratch
WORKDIR /app
COPY --from=build /out/patternify-bot .

CMD ["/app/patternify-bot"]
