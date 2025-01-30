--- Migrate UP ProjectUserMap

CREATE ProjectUserMap (
	UserID REFERENCES User(UserID),
	ProjectID REFERENCES Project(ProjectID)
);

