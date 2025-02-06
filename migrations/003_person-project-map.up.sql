--- Migrate UP PersonProjectMap

CREATE TABLE PersonProjectMap (
	PersonID INTEGER references Person(PersonID),
	ProjectID INTEGER references Project(ProjectID),
	-- true:: diese Person ist Admin für dieses Projekt.
	IsProjectAdmin BOOL NOT NULL DEFAULT FALSE
);

