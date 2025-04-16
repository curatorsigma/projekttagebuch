--- insert three users and two projects

INSERT INTO Person (PersonName, PersonSurname, PersonFirstname) VALUES ('Adam', 'Abramovich', 'Adam');
INSERT INTO Person (PersonName, PersonSurname, PersonFirstname) VALUES ('Beth', 'Beliar', 'Beth');
INSERT INTO Person (PersonName, PersonSurname, PersonFirstname) VALUES ('Gamaliel', 'Germof', 'Gamaliel');

INSERT INTO Project (ProjectName, ProjectRoomId) VALUES ('1Basil', 'matrix-id');
INSERT INTO Project (ProjectName, ProjectRoomId) VALUES ('2Basil', 'matrix-id');

INSERT INTO PersonProjectMap (ProjectID, PersonID, IsProjectAdmin) VALUES (1, 1, TRUE);
INSERT INTO PersonProjectMap (ProjectID, PersonID, IsProjectAdmin) VALUES (1, 2, FALSE);

INSERT INTO PersonProjectMap (ProjectID, PersonID, IsProjectAdmin) VALUES (2, 1, FALSE);
INSERT INTO PersonProjectMap (ProjectID, PersonID, IsProjectAdmin) VALUES (2, 3, TRUE);
