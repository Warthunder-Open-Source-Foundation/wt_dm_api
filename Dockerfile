FROM docker.io/rust:1.89 as builder

WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock ./
COPY ./src ./src
COPY assets ./assets

RUN cargo build --release

FROM docker.io/archlinux
WORKDIR /usr/src/app
COPY --from=builder /usr/src/app/target/release/wt_dm_api .
#EXPOSE 3000

CMD ["./wt_dm_api"]