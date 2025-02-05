--- Migrate UP PersonProjectMap

CREATE TABLE PersonProjectMap (
	PersonID INTEGER references Person(PersonID),
	ProjectID INTEGER references Project(ProjectID)
);

