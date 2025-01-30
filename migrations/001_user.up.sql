--- Migrate UP User

CREATE TABLE User (
	UserID INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	UserName TEXT NOT NULL,
	--- UTC. Wann wurde dieser User das letzt mal aus LDAP synchronisiert
	--- Wird gesetzt, wenn geprüft wurde, dass der User mit diesem namen an dieser LdapLocation existiert
	--- und HasWritePrivilege gesetzt wurde
	LastSync TIMESTAMP NOT NULL,
	--- Dieser User hat Schreibrecht (kann Projekte erstellen)
	HasWritePrivilege BOOL NOT NULL DEFAULT FALSE,
);
