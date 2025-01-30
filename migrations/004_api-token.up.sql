--- Migrate UP ApiToken

CREATE TABLE ApiToken (
	ApiTokenID INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	UserID REFERENCES User(UserID),
	TokenHash TEXT NOT NULL
);
