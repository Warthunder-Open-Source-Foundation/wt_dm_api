FROM docker.io/rust:1.81 as builder

RUN rustup default nightly

WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock ./
COPY ./src ./src
COPY assets ./

RUN cargo build --release

FROM docker.io/archlinux
WORKDIR /usr/src/app
COPY --from=builder /usr/src/app/target/release/wt_dm_api .
#EXPOSE 3000

CMD ["./wt_dm_api"]