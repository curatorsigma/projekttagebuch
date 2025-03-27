# normal rust builder
FROM rust:1.83-alpine AS builder
RUN apk add --no-cache build-base
WORKDIR /usr/src/projekttagebuch
COPY . .
RUN cargo build --release

FROM alpine:latest
# we need to inject LDAP certificates once the container is already running.
# For this, we add them as configs in docker-compose, under /usr/local/share/ca-certificates/
# and we run update-ca-certificates before starting the actual project
RUN apk add --no-cache ca-certificates
WORKDIR projekttagebuch/
COPY --from=builder /usr/src/projekttagebuch/target/release/projekttagebuch ./
CMD ["/bin/ash", "-c", "update-ca-certificates;./projekttagebuch"]
