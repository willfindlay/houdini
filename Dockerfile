FROM rust:1.59 as builder
WORKDIR /usr/src
RUN rustup default nightly
RUN rustup target add x86_64-unknown-linux-musl

RUN mkdir -p houdini/src houdini/bin
RUN cargo init temp && cp temp/src/main.rs houdini/bin/houdini.rs
WORKDIR /usr/src/houdini
RUN touch src/lib.rs bin/houdini.rs

COPY Cargo.toml Cargo.lock ./
RUN cargo build --release

RUN rm -rf src bin
COPY src ./src
COPY bin ./bin
RUN cargo install --target x86_64-unknown-linux-musl --path .

FROM scratch
COPY --from=builder /usr/local/cargo/bin/houdini .
ENV PATH .
USER 9999
CMD ["./houdini"]
