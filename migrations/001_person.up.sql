--- Migrate UP Person

CREATE TABLE Person (
	PersonID INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	PersonName TEXT NOT NULL,
	--- UTC. Wann wurde diese Person das letzt mal aus LDAP synchronisiert
	--- Wird gesetzt, wenn geprüft wurde, dass die Person mit diesem namen an dieser LdapLocation existiert
	--- und HasWritePrivilege gesetzt wurde
	LastSync TIMESTAMP NOT NULL,
	--- Diese Person hat Schreibrecht (kann Projekte erstellen)
	HasWritePrivilege BOOL NOT NULL DEFAULT FALSE
);
