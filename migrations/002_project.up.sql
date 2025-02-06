--- Migrate UP Project

CREATE TABLE Project (
	ProjectID INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	--- Name des Projekts
	ProjectName TEXT NOT NULL
);

