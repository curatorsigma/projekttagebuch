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

