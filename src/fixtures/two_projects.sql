--- insert three users and two projects

INSERT INTO Person (PersonName) VALUES ('Adam');
INSERT INTO Person (PersonName) VALUES ('Beth');
INSERT INTO Person (PersonName) VALUES ('Gamaliel');

INSERT INTO Project (ProjectName) VALUES ('1Basil');
INSERT INTO Project (ProjectName) VALUES ('2Basil');

INSERT INTO PersonProjectMap (ProjectID, PersonID, IsProjectAdmin) VALUES (1, 1, TRUE);
INSERT INTO PersonProjectMap (ProjectID, PersonID, IsProjectAdmin) VALUES (1, 2, FALSE);

INSERT INTO PersonProjectMap (ProjectID, PersonID, IsProjectAdmin) VALUES (2, 1, FALSE);
INSERT INTO PersonProjectMap (ProjectID, PersonID, IsProjectAdmin) VALUES (2, 3, TRUE);
