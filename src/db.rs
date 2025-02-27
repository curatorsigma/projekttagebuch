//! Low level Database primitives

use sqlx::{PgPool, Row};
use tracing::{info, trace, warn};

use crate::types::{HasID, NoID, Person, Project, UserPermission};

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
    CannotSelectPersons(sqlx::Error),
    CannotSelectPersonByExactName(sqlx::Error),
    CannotDeletePerson(sqlx::Error, String),
    CannotUpdateGlobalPermissions(sqlx::Error, String),

    // DATA Errors
    ProjectDoesNotExist(i32, String),
    ProjectNotUnique(i32),
    PPMapEntryHasNoCorrespondingProject(i32, i32),
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
            Self::CannotSelectPersons(x) => {
                write!(f, "Unable to select persons: {x}")
            }
            Self::CannotSelectPersonByExactName(x) => {
                write!(f, "Unable to select person with exact name: {x}")
            }
            Self::CannotDeletePerson(x, y) => {
                write!(f, "Unable to delete person with id {x} and name {y}.")
            }
            Self::CannotUpdateGlobalPermissions(x, y) => {
                write!(f, "Cannot update global permissions for user {}: {}.", x, y)
            }
            Self::ProjectDoesNotExist(x, y) => {
                write!(f, "The project with id {x}, name {y} does not exist.")
            }
            Self::ProjectNotUnique(x) => {
                write!(f, "The project with id {x} exists multiple times.")
            }
            Self::PPMapEntryHasNoCorrespondingProject(person, project) => {
                write!(f, "Person {person} is mapped to Project {project} but that project does not exist.")
            }
        }
    }
}
impl std::error::Error for DBError {}

/// Get a list of projects
pub async fn get_projects(pool: PgPool) -> Result<Vec<Project<HasID>>, DBError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(DBError::CannotStartTransaction)?;

    // first get all projects (required if projects are empty)
    let rows = sqlx::query!("SELECT ProjectID, ProjectName FROM Project;")
        .fetch_all(&mut *tx)
        .await
        .map_err(DBError::CannotSelectProjects)?;
    let mut result = rows
        .into_iter()
        .map(|r| Project::new(r.projectid, r.projectname))
        .collect::<Vec<Project<HasID>>>();

    // Now get all users part of any projects
    let rows = sqlx::query!(
        "SELECT Project.ProjectID, Project.ProjectName, Person.PersonID, Person.PersonName, Person.PersonSurname, Person.PersonFirstname, Person.IsGlobalAdmin, PersonProjectMap.IsProjectAdmin
            FROM Project
        INNER JOIN PersonProjectMap
            ON Project.ProjectID = PersonProjectMap.ProjectID
        INNER JOIN Person
            ON PersonProjectMap.PersonID = Person.PersonID;"
    )
        .fetch_all(&mut *tx)
        .await
        .map_err(DBError::CannotSelectProjects)?;

    'row: for row in rows {
        let person = Person::new(
            row.personid,
            row.personname,
            UserPermission::new_from_is_admin(row.isglobaladmin),
            row.personsurname,
            row.personfirstname,
        );

        for project in result.iter_mut() {
            if project.project_id() == row.projectid {
                project.add_member(
                    person,
                    UserPermission::new_from_is_admin(row.isprojectadmin),
                );
                continue 'row;
            };
        }
        // no existing project fit - create a new one
        // This should not be possible because we have selected all projects in the same transaction
        warn!("Found no project for a PersonProjectMap entry. Check DB data integrity!");
        warn!("Person {}, Project {} is mapped but Project does not exist.", row.personid, row.projectid);
        return Err(DBError::PPMapEntryHasNoCorrespondingProject(row.personid, row.projectid))
    }
    // deliberately no transaction commit because we have not written anything in it
    Ok(result)
}

/// Add a new project
pub(crate) async fn add_project(pool: PgPool, project: Project<NoID>) -> Result<Project<HasID>, DBError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(DBError::CannotStartTransaction)?;

    let new_id = sqlx::query!(
        "INSERT INTO Project (ProjectName) VALUES ($1) RETURNING ProjectID;",
        project.name
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(DBError::CannotInsertProject)?;
    let idd_project = project.set_id(new_id.projectid);

    for member in idd_project.members.iter() {
        sqlx::query!("INSERT INTO PersonProjectMap (PersonID, ProjectID, IsProjectAdmin) VALUES ($1, $2, $3);",
            member.0.person_id(),
            new_id.projectid,
            member.1.is_admin(),
            )
            .execute(&mut *tx)
            .await
            .map_err(DBError::CannotInsertPPMap)?;
    }

    tx.commit()
        .await
        .map_err(DBError::CannotCommitTransaction)?;
    Ok(idd_project)
}

/// Get a project from known ID
pub(crate) async fn get_project(pool: PgPool, id: i32) -> Result<Option<Project<HasID>>, DBError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(DBError::CannotStartTransaction)?;

    // first get all projects (required if projects are empty)
    let rows = sqlx::query!(
            "SELECT ProjectID, ProjectName FROM Project WHERE ProjectID = $1;",
            id,
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(DBError::CannotSelectProjects)?;
    let mut project = match rows {
        None => {
            trace!("Project {id} does not exist.");
            return Ok(None);
        }
        Some(x) => {
            Project::<HasID>::new(x.projectid, x.projectname)
        }
    };

    let rows = sqlx::query!(
        "SELECT Project.ProjectID, Project.ProjectName, Person.PersonID, Person.PersonName, Person.PersonFirstname, Person.PersonSurname, Person.IsGlobalAdmin, PersonProjectMap.IsProjectAdmin
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
    'row: for row in rows {
        let person = Person::new(
            row.personid,
            row.personname,
            UserPermission::new_from_is_admin(row.isglobaladmin),
            row.personsurname,
            row.personfirstname,
        );

        if project.project_id() == row.projectid {
            project.add_member(
                person,
                UserPermission::new_from_is_admin(row.isprojectadmin),
            );
            continue 'row;
        };
        // no existing project fit - create a new one
        // This should not be possible because we have selected all projects in the same transaction
        warn!("Found no project for a PersonProjectMap entry. Check DB data integrity!");
        warn!("Person {}, Project {} is mapped but Project does not exist.", row.personid, row.projectid);
        return Err(DBError::PPMapEntryHasNoCorrespondingProject(row.personid, row.projectid))
    };
    Ok(Some(project))
}

/// remove the given persons from the given project
async fn remove_members(
    pool: PgPool,
    project_id: i32,
    members_to_remove: &[&Person<HasID>],
) -> Result<(), DBError> {
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
    }

    tx.commit()
        .await
        .map_err(DBError::CannotCommitTransaction)?;
    Ok(())
}

/// remove the given persons from the given project
async fn add_members(
    pool: PgPool,
    project_id: i32,
    members_to_add: &[(&Person<HasID>, &UserPermission)],
) -> Result<(), DBError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(DBError::CannotStartTransaction)?;
    for (mem, permission) in members_to_add.iter() {
        sqlx::query!(
            "INSERT INTO PersonProjectMap (PersonID, ProjectID, IsProjectAdmin) VALUES ($1, $2, $3);",
            mem.person_id(),
            project_id,
            permission.is_admin(),
        )
        .execute(&mut *tx)
        .await
        .map_err(DBError::CannotInsertPPMap)?;
    }

    tx.commit()
        .await
        .map_err(DBError::CannotCommitTransaction)?;
    Ok(())
}

/// Set(overwrite) the members of a project
async fn update_project_members(pool: PgPool, project: Project<HasID>) -> Result<(), DBError> {
    let old_project = get_project(pool.clone(), project.project_id())
        .await?
        .ok_or(DBError::ProjectDoesNotExist(
            project.project_id(),
            project.name.clone(),
        ))?;

    let members_to_remove = old_project
        .members
        .iter()
        .filter_map(|(m, _)| {
            if project.members.iter().any(|(n, _)| m == n) {
                Some(m)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    // this also tracks admin-status
    let members_to_add = project
        .members
        .iter()
        .filter_map(|(m, is_adm)| {
            if old_project.members.iter().any(|(n, _)| m == n) {
                Some((m, is_adm))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

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
        "INSERT INTO Person (PersonName, PersonSurname, PersonFirstname, IsGlobalAdmin) VALUES ($1, $2, $3, $4) RETURNING PersonID",
        &person.name,
        &person.surname,
        &person.firstname,
        &person.is_global_admin(),
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(DBError::CannotInsertPerson)?;

    Ok(Person::<HasID>::new(
        new_id_result.personid,
        person.name,
        person.global_permission,
        person.surname,
        person.firstname,
    ))
}

/// Get a person from the DB by name (exact)
pub async fn get_person(pool: PgPool, name: &str) -> Result<Option<Person<HasID>>, DBError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(DBError::CannotStartTransaction)?;
    let id_result = sqlx::query!(
        "SELECT PersonID, PersonFirstname, PersonSurname, IsGlobalAdmin from Person WHERE PersonName LIKE $1;",
        name,
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(DBError::CannotSelectPersonByExactName)?;

    match id_result {
        Some(x) => Ok(Some(Person::<HasID>::new(
            x.personid,
            name.to_owned(),
            UserPermission::new_from_is_admin(x.isglobaladmin),
            x.personsurname,
            x.personfirstname,
        ))),
        None => Ok(None),
    }
}

/// Get all persons from the DB
async fn get_all_persons(pool: PgPool) -> Result<Vec<Person<HasID>>, DBError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(DBError::CannotStartTransaction)?;
    let res = sqlx::query!("SELECT PersonID, PersonName, PersonSurname, PersonFirstname, IsGlobalAdmin from Person;",)
        .fetch_all(&mut *tx)
        .await
        .map_err(DBError::CannotSelectPersons)?;
    Ok(res
        .into_iter()
        .map(|r| {
            Person::new(
                r.personid,
                r.personname,
                UserPermission::new_from_is_admin(r.isglobaladmin),
                r.personsurname,
                r.personfirstname,
            )
        })
        .collect::<Vec<_>>())
}

/// Update users in the DB such that exactly these users exist with these permissions.
///
/// NOTE: Permissions are global permissions here, not project-based.
pub async fn update_users(pool: PgPool, users: Vec<Person<NoID>>) -> Result<(), DBError> {
    trace!("Want these users to be in the db: {users:?}");
    // first get users from DB to calculate diff
    let users_in_db = get_all_persons(pool.clone()).await?;

    let users_to_delete = users_in_db
        .iter()
        .filter(|p| users.iter().all(|q| p.name != q.name));
    let mut tx = pool
        .begin()
        .await
        .map_err(DBError::CannotStartTransaction)?;
    for user in users_to_delete {
        sqlx::query!("DELETE FROM Person WHERE PersonID = $1;", user.person_id(),)
            .execute(&mut *tx)
            .await
            .map_err(|e| DBError::CannotDeletePerson(e, user.name.clone()))?;
        info!(
            "Removed user {} from DB. They no longer exist in LDAP.",
            user.name
        )
    }
    for user in users {
        // get user by name
        let person = sqlx::query!(
            "SELECT PersonID, IsGlobalAdmin from Person WHERE PersonName LIKE $1;",
            user.name,
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(DBError::CannotSelectPersonByExactName)?;
        match person {
            None => {
                sqlx::query!(
                    "INSERT INTO Person (PersonName, PersonSurname, PersonFirstname, IsGlobalAdmin) VALUES ($1, $2, $3, $4);",
                    user.name,
                    user.surname,
                    user.firstname,
                    user.is_global_admin()
                )
                .execute(&mut *tx)
                .await
                .map_err(DBError::CannotInsertPerson)?;
                info!(
                    "Inserted new user {} into DB as {}.",
                    user.name,
                    user.is_global_admin()
                );
            }
            Some(row) => {
                // update admin status
                let old_is_global_admin = row.isglobaladmin;
                if old_is_global_admin == user.is_global_admin() {
                    trace!("User {}: Still exists, admin status unchanged.", user.name);
                } else {
                    sqlx::query!(
                        "UPDATE Person SET IsGlobalAdmin = $1 WHERE PersonName = $2;",
                        user.is_global_admin(),
                        user.name,
                    )
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DBError::CannotUpdateGlobalPermissions(e, user.name.to_owned()))?;
                    info!(
                        "Global Admin Status for {} changed. Is now: {}.",
                        user.name, user.global_permission
                    );
                };
                // update name
                sqlx::query!(
                    "UPDATE Person SET PersonSurname = $1, PersonFirstname = $2 WHERE PersonName = $3;",
                    user.surname,
                    user.firstname,
                    user.name,
                )
                .execute(&mut *tx)
                .await
                .map_err(|e| DBError::CannotUpdateGlobalPermissions(e, user.name.to_owned()))?;
                trace!("User {} Firstname set to: {}, Surname set to: {}", user.name, user.firstname, user.surname);
            }
        };
    }

    tx.commit()
        .await
        .map_err(DBError::CannotCommitTransaction)?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use sqlx::PgPool;

    #[sqlx::test(fixtures("empty"))]
    async fn test_add_person(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let person = Person::<NoID> {
            person_id: NoID::default(),
            name: "John Doe".to_owned(),
            global_permission: UserPermission::User,
            firstname: "John".to_owned(),
            surname: "Doe".to_owned(),
        };
        add_person(pool, person).await.unwrap();
        Ok(())
    }

    #[sqlx::test(fixtures("two_projects"))]
    async fn test_get_projects(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let projects = get_projects(pool).await.unwrap();
        assert_eq!(projects.len(), 2);
        Ok(())
    }

    #[sqlx::test(fixtures("empty_project"))]
    async fn test_get_projects_no_users(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let projects = get_projects(pool).await.unwrap();
        assert_eq!(projects.len(), 1);
        Ok(())
    }

    #[sqlx::test(fixtures("two_projects"))]
    async fn test_add_project(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let new_project: Project<NoID> = Project::new((), "some new name".to_owned());
        let idd_project = add_project(pool, new_project).await.unwrap();
        assert_eq!(idd_project.project_id(), 3);
        Ok(())
    }

    #[sqlx::test(fixtures("two_projects"))]
    async fn test_add_members(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let project_id = 1;
        let adam = Person::new(1, "Adam".to_owned(), UserPermission::Admin, "Adam".to_owned(), "Abrahamovitch".to_owned());
        let persons = vec![(&adam, &UserPermission::User)];
        add_members(pool.clone(), project_id, &persons).await?;

        let project = get_project(pool.clone(), project_id).await?.unwrap();
        assert_eq!(project.members.len(), 3);
        Ok(())
    }

    #[sqlx::test(fixtures("two_projects"))]
    async fn test_remove_members(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let project_id = 1;
        let adam = Person::new(1, "Adam".to_owned(), UserPermission::Admin, "Adam".to_owned(), "Abrahamovitch".to_owned());
        let persons = vec![&adam];
        remove_members(pool.clone(), project_id, &persons).await?;

        let project = get_project(pool.clone(), project_id).await?.unwrap();
        assert_eq!(project.members.len(), 1);
        Ok(())
    }

    #[sqlx::test(fixtures("two_projects"))]
    async fn test_update_members(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let project_id = 1;
        let david = Person::<NoID>::new((), "David".to_owned(), UserPermission::Admin, "David".to_owned(), "Descartes".to_owned());
        let hanna = Person::<NoID>::new((), "Hanna".to_owned(), UserPermission::User, "Hanna".to_owned(), "Herakleia".to_owned());
        let samuel = Person::<NoID>::new((), "Samuel".to_owned(), UserPermission::User, "Samuel".to_owned(), "Shmuelov".to_owned());

        let david = add_person(pool.clone(), david).await?;
        let hanna = add_person(pool.clone(), hanna).await?;
        let samuel = add_person(pool.clone(), samuel).await?;

        let mut basil_1 = Project::new(1, "1Basil".to_owned());
        basil_1.members.push((david, UserPermission::User));
        basil_1.members.push((hanna, UserPermission::Admin));
        basil_1.members.push((samuel, UserPermission::Admin));

        update_project_members(pool.clone(), basil_1).await?;

        let project = get_project(pool.clone(), project_id).await?.unwrap();
        assert_eq!(project.members.len(), 2);

        Ok(())
    }

    #[sqlx::test(fixtures("two_projects"))]
    async fn test_update_users(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let david = Person::<NoID>::new((), "David".to_owned(), UserPermission::Admin, "David".to_owned(), "Descartes".to_owned());
        let hanna = Person::<NoID>::new((), "Hanna".to_owned(), UserPermission::User, "Hanna".to_owned(), "Herakleia".to_owned());
        let samuel = Person::<NoID>::new((), "Samuel".to_owned(), UserPermission::User, "Samuel".to_owned(), "Shmuelov".to_owned());

        update_users(pool.clone(), vec![david, hanna, samuel]).await?;
        let persons = get_all_persons(pool.clone()).await?;
        assert_eq!(persons.len(), 3);

        Ok(())
    }
}
