//! Low level Database primitives

use sqlx::{postgres::PgRow, PgPool, Postgres, Row};

use crate::types::{HasID, NoID, Person, Project};

#[derive(Debug)]
pub(crate) enum DBError {
    // Engine or uncertain
    CannotAuthenticate(sqlx::Error),
    CannotStartTransaction(sqlx::Error),
    CannotCommitTransaction(sqlx::Error),
    CannotRollbackTransaction(sqlx::Error),
    CannotInsertPerson(sqlx::Error),
    CannotInsertProject(sqlx::Error),
    CannotInsertPPMap(sqlx::Error),
    CannotSelectProjects(sqlx::Error),
    CannotSelectPersonByExactName(sqlx::Error),

    // DATA Errors
    ProjectDoesNotExist(i32, String),
    ProjectNotUnique(i32),
}
impl core::fmt::Display for DBError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CannotAuthenticate(x) => {
                write!(f, "Can not authenticate to DB: {x}")
            }
            Self::CannotStartTransaction(x) => {
                write!(f, "Unable to start transaction: {x}")
            }
            Self::CannotCommitTransaction(x) => {
                write!(f, "Unable to commit transaction: {x}")
            }
            Self::CannotRollbackTransaction(x) => {
                write!(f, "Unable to rollback transaction: {x}")
            }
            Self::CannotInsertPerson(x) => {
                write!(f, "Unable to insert a person: {x}")
            }
            Self::CannotInsertProject(x) => {
                write!(f, "Unable to insert a project: {x}")
            }
            Self::CannotInsertPPMap(x) => {
                write!(f, "Unable to insert a Person-Project-Mapping: {x}")
            }
            Self::CannotSelectProjects(x) => {
                write!(f, "Unable to select projects: {x}")
            }
            Self::CannotSelectPersonByExactName(x) => {
                write!(f, "Unable to select person with exact name: {x}")
            }
            Self::ProjectDoesNotExist(x, y) => {
                write!(f, "The project with id {x}, name {y} does not exist.")
            }
            Self::ProjectNotUnique(x) => {
                write!(f, "The project with id {x} exists multiple times.")
            }
        }
    }
}
impl std::error::Error for DBError {}

/// Get a list of projects
async fn get_projects(pool: PgPool) -> Result<Vec<Project<HasID>>, DBError> {
    let rows = sqlx::query!(
        "SELECT Project.ProjectID, Project.ProjectName, Person.PersonID, Person.PersonName, PersonProjectMap.IsProjectAdmin
            FROM Project
        INNER JOIN PersonProjectMap
            ON Project.ProjectID = PersonProjectMap.ProjectID
        INNER JOIN Person
            ON PersonProjectMap.PersonID = Person.PersonID;"
    )
        .fetch_all(&pool)
        .await
        .map_err(DBError::CannotSelectProjects)?;
    let mut result: Vec<Project<HasID>> = vec![];
    'row: for row in rows {
        let person = Person::new(row.personid, row.personname);

        for project in result.iter_mut() {
            if project.project_id() == row.projectid {
                project.add_member(person, row.isprojectadmin);
                continue 'row;
            };
        };
        // no existing project fit - create a new one
        let mut project = Project::new(row.projectid, row.projectname);
        project.add_member(person, row.isprojectadmin);
        result.push(project);
    };
    Ok(result)
}

/// Add a new project
async fn add_project(pool: PgPool, project: Project<NoID>) -> Result<Project<HasID>, DBError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(DBError::CannotStartTransaction)?;

    let new_id = sqlx::query!("INSERT INTO Project (ProjectName) VALUES ($1) RETURNING ProjectID;",
                  project.name)
        .fetch_one(&mut *tx)
        .await
        .map_err(DBError::CannotInsertProject)?;
    let idd_project = project.set_id(new_id.projectid);

    for member in idd_project.members.iter() {
        sqlx::query!("INSERT INTO PersonProjectMap (PersonID, ProjectID, IsProjectAdmin) VALUES ($1, $2, $3);",
            member.0.person_id(),
            new_id.projectid,
            member.1,
            )
            .execute(&mut *tx)
            .await
            .map_err(DBError::CannotInsertPPMap)?;
    };

    tx.commit()
        .await
        .map_err(DBError::CannotCommitTransaction)?;
    Ok(idd_project)
}

/// Get a project from known ID
async fn get_project(pool: PgPool, id: i32) -> Result<Option<Project<HasID>>, DBError> {
    let rows = sqlx::query!(
        "SELECT Project.ProjectID, Project.ProjectName, Person.PersonID, Person.PersonName, PersonProjectMap.IsProjectAdmin
            FROM Project
        INNER JOIN PersonProjectMap
            ON Project.ProjectID = PersonProjectMap.ProjectID
        INNER JOIN Person
            ON PersonProjectMap.PersonID = Person.PersonID
        WHERE
            Project.ProjectID = $1;",
        id,
    )
        .fetch_all(&pool)
        .await
        .map_err(DBError::CannotSelectProjects)?;
    let mut result: Vec<Project<HasID>> = vec![];
    'row: for row in rows {
        let person = Person::new(row.personid, row.personname);

        for project in result.iter_mut() {
            if project.project_id() == row.projectid {
                project.add_member(person, row.isprojectadmin);
                continue 'row;
            };
        };
        // no existing project fit - create a new one
        let mut project = Project::new(row.projectid, row.projectname);
        project.add_member(person, row.isprojectadmin);
        result.push(project);
    };
    match result.len() {
        0 => {
            Ok(None)
        }
        1 => {
            Ok(result.pop())
        }
        _ => {
            Err(DBError::ProjectNotUnique(id))
        }
    }
}

/// remove the given persons from the given project
async fn remove_members(pool: PgPool, project_id: i32, members_to_remove: &[&Person<HasID>]) -> Result<(), DBError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(DBError::CannotStartTransaction)?;
    for mem in members_to_remove.iter() {
        sqlx::query!(
            "DELETE FROM PersonProjectMap WHERE PersonID = $1 AND ProjectID = $2;",
            mem.person_id(),
            project_id,
        )
        .execute(&mut *tx)
        .await
        .map_err(DBError::CannotInsertPPMap)?;
    };

    tx.commit()
        .await
        .map_err(DBError::CannotCommitTransaction)?;
    Ok(())
}

/// remove the given persons from the given project
async fn add_members(pool: PgPool, project_id: i32, members_to_add: &[(&Person<HasID>, &bool)]) -> Result<(), DBError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(DBError::CannotStartTransaction)?;
    for (mem, is_adm) in members_to_add.iter() {
        sqlx::query!(
            "INSERT INTO PersonProjectMap (PersonID, ProjectID, IsProjectAdmin) VALUES ($1, $2, $3);",
            mem.person_id(),
            project_id,
            is_adm,
        )
        .execute(&mut *tx)
        .await
        .map_err(DBError::CannotInsertPPMap)?;
    };

    tx.commit()
        .await
        .map_err(DBError::CannotCommitTransaction)?;
    Ok(())
}

/// Set(overwrite) the members of a project
async fn update_project_members(pool: PgPool, project: Project<HasID>) -> Result<(), DBError> {
    let old_project = get_project(pool.clone(), project.project_id()).await?.ok_or(DBError::ProjectDoesNotExist(project.project_id(), project.name.clone()))?;

    let members_to_remove = old_project.members.iter().filter_map(
        |(m, _)| {
        if project.members.iter().any(|(n, _)| m == n) {
            Some(m)
        } else {
            None
        }
        }).collect::<Vec<_>>();
    // this also tracks admin-status
    let members_to_add = project.members.iter().filter_map(
        |(m, is_adm)| {
        if old_project.members.iter().any(|(n, _)| m == n) {
            Some((m, is_adm))
        } else {
            None
        }
        }).collect::<Vec<_>>();

    // delete stale members
    remove_members(pool.clone(), project.project_id(), &members_to_remove).await?;
    // add new members
    add_members(pool.clone(), project.project_id(), &members_to_add).await?;

    Ok(())
}

/// Add a person.
async fn add_person(pool: PgPool, person: Person<NoID>) -> Result<Person<HasID>, DBError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(DBError::CannotStartTransaction)?;
    let new_id_result = sqlx::query!(
        "INSERT INTO Person (PersonName) VALUES ($1) RETURNING PersonID",
        &person.name,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(DBError::CannotInsertPerson)?;

    let new_id: i32 = new_id_result.personid;
    Ok(Person::<HasID>::new(new_id, person.name))
}

/// Get a person from the DB by name (exact)
async fn get_person(pool: PgPool, name: &str) -> Result<Option<Person<HasID>>, DBError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(DBError::CannotStartTransaction)?;
    let id_result = sqlx::query!(
        "SELECT PersonID from Person WHERE PersonName LIKE $1;",
        name,
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(DBError::CannotSelectPersonByExactName)?;

    let id: i32 = match id_result {
        Some(x) => { x.personid }
        None => {return Ok(None);}
    };
    Ok(Some(Person::<HasID>::new(id, name.to_owned())))
}

#[cfg(test)]
mod test {
    use super::*;
    use sqlx::PgPool;

    #[sqlx::test(fixtures("empty"))]
    async fn test_add_person(
        pool: PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let person = Person::<NoID> {
            person_id: NoID::default(),
            name: "John Doe".to_owned(),
        };
        add_person(pool, person).await.unwrap();
        Ok(())
    }


    #[sqlx::test(fixtures("two_projects"))]
    async fn test_get_projects(
        pool: PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let projects = get_projects(pool).await.unwrap();
        assert_eq!(projects.len(), 2);
        Ok(())
    }

    #[sqlx::test(fixtures("two_projects"))]
    async fn test_add_project(
        pool: PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let new_project: Project::<NoID> = Project::new((), "some new name".to_owned());
        let idd_project = add_project(pool, new_project).await.unwrap();
        assert_eq!(idd_project.project_id(), 3);
        Ok(())
    }

    #[sqlx::test(fixtures("two_projects"))]
    async fn test_add_members(
        pool: PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let project_id = 1;
        let adam = Person::new(1, "Adam".to_owned());
        let persons = vec![(&adam, &false)];
        add_members(pool.clone(), project_id, &persons).await?;

        let project = get_project(pool.clone(), project_id).await?.unwrap();
        assert_eq!(project.members.len(), 3);
        Ok(())
    }

    #[sqlx::test(fixtures("two_projects"))]
    async fn test_remove_members(
        pool: PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let project_id = 1;
        let adam = Person::new(1, "Adam".to_owned());
        let persons = vec![&adam];
        remove_members(pool.clone(), project_id, &persons).await?;

        let project = get_project(pool.clone(), project_id).await?.unwrap();
        assert_eq!(project.members.len(), 1);
        Ok(())
    }

    #[sqlx::test(fixtures("two_projects"))]
    async fn test_update_members(
        pool: PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let project_id = 1;
        let david = Person::<NoID>::new((), "David".to_owned());
        let hanna = Person::<NoID>::new((), "Hanna".to_owned());
        let samuel = Person::<NoID>::new((), "Samuel".to_owned());

        let david = add_person(pool.clone(), david).await?;
        let hanna = add_person(pool.clone(), hanna).await?;
        let samuel = add_person(pool.clone(), samuel).await?;

        let mut basil_1 = Project::new(1, "1Basil".to_owned());
        basil_1.members.push((david, false));
        basil_1.members.push((hanna, true));
        basil_1.members.push((samuel, true));

        update_project_members(pool.clone(), basil_1).await?;

        let project = get_project(pool.clone(), project_id).await?.unwrap();
        assert_eq!(project.members.len(), 2);

        Ok(())
    }
}
