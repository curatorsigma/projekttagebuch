FROM rust:1.80-alpine AS builder
RUN apk add --no-cache build-base
WORKDIR /usr/src/projekttagebuch
COPY . .
RUN cargo build --release
CMD ["projekttagebuch"]

FROM alpine:latest
WORKDIR projekttagebuch/
COPY --from=builder /usr/src/projekttagebuch/target/release/projekttagebuch ./
CMD ["./projekttagebuch"]

