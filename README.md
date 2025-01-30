# Projekttagebuch

## Architektur
Dieser Service besteht aus:
- einem Webserver
- Einem Synchronisationsservice (einzelner thread), der periodisch user aus LDAP und Räume nach Matrix synchronisiert

### Anschlüsse an externe Services
#### Postgres
Dieser Service speichert Daten in einer PostgreSQL Datenbank.
#### Matrix
Dieser Service nutzt die API eines externen Matrix-Servers.

## Setup
Dieser Service läuft standardmäßig in Docker

## Konfiguration
Es gibt ein `.toml` file, in dem alle config drin steht
TODO: wo ist das config-file?

# TLS for LDAP
- the LDAP servers TLS cert must be trusted
- to do this, manually cat the RootCA into /etc/ssl/certs/ca-certificates.crt (docker image can do this)
- alternatively, try embedding the TLS cert (not in the build step) and have update-ca-certificates as part of the startup command
- rust::ldap3 uses the rustls_native_certs, so it uses the native trusted certs of the system
https://github.com/inejge/ldap3/blob/master/src/conn.rs
https://github.com/rustls/rustls-native-certs
https://stackoverflow.com/questions/67231714/how-to-add-trusted-root-ca-to-docker-alpine

