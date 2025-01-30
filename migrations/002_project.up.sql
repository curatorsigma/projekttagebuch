--- Migrate UP Project

CREATE TABLE Project (
	ProjectID INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	--- Name des Projekts
	ProjectName TEXT NOT NULL,
	--- UTC: wann wurde der Matrix-Raum dieses Projekts das letzte mal synchronisiert
	RoomLastSync TIMESTAMP NOT NULL
);

