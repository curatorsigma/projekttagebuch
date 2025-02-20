--- Migrate UP PersonProjectMap

CREATE TABLE PersonProjectMap (
	PersonID INTEGER references Person(PersonID) ON DELETE CASCADE,
	ProjectID INTEGER references Project(ProjectID) ON DELETE RESTRICT,
	-- true:: diese Person ist Admin für dieses Projekt.
	IsProjectAdmin BOOL NOT NULL DEFAULT FALSE
);

