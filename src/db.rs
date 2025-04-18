//! Low level Database primitives

use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use tracing::{info, trace, warn};

use crate::types::{DbNoMatrix, FullId, MatrixNoDb, NoId, Person, Project, UserPermission};

#[derive(Debug)]
pub(crate) enum DBError {
    // Engine or uncertain
    CannotStartTransaction(sqlx::Error),
    CannotCommitTransaction(sqlx::Error),
    CannotInsertPerson(sqlx::Error),
    CannotInsertProject(sqlx::Error),
    CannotInsertPPMap(sqlx::Error),
    CannotSelectProjects(sqlx::Error),
    CannotSelectPersons(sqlx::Error),
    CannotSelectPersonByExactName(sqlx::Error),
    CannotDeletePerson(sqlx::Error, String),
    CannotUpdateGlobalPermissions(sqlx::Error, String),
    CannotUpdateFirstname(sqlx::Error, String),
    CannotUpdateSurname(sqlx::Error, String),
    CannotSelectSimilarNames(sqlx::Error),
    CannotRemoveMember(sqlx::Error),
    CannotUpdateMemberPermission(sqlx::Error),
    CannotChangeProjectName(sqlx::Error),

    // DATA Errors
    ProjectDoesNotExist(i32, String),
    PPMapEntryHasNoCorrespondingProject(i32, i32),
}
impl core::fmt::Display for DBError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CannotStartTransaction(x) => {
                write!(f, "Unable to start transaction: {x}")
            }
            Self::CannotCommitTransaction(x) => {
                write!(f, "Unable to commit transaction: {x}")
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
            Self::CannotRemoveMember(x) => {
                write!(f, "Unable to delete person: {x}.")
            }
            Self::CannotUpdateMemberPermission(x) => {
                write!(f, "Unable to update permission for person: {x}.")
            }
            Self::CannotUpdateGlobalPermissions(x, y) => {
                write!(f, "Cannot update global permissions for user {}: {}.", x, y)
            }
            Self::CannotUpdateFirstname(x, y) => {
                write!(f, "Cannot update firstname for user {}: {}.", x, y)
            }
            Self::CannotUpdateSurname(x, y) => {
                write!(f, "Cannot update surname for user {}: {}.", x, y)
            }
            Self::CannotSelectSimilarNames(x) => {
                write!(f, "Cannot select similar names: {}.", x)
            }
            Self::CannotChangeProjectName(x) => {
                write!(f, "Cannot rename a project: {x}")
            }
            Self::ProjectDoesNotExist(x, y) => {
                write!(f, "The project with id {x}, name {y} does not exist.")
            }
            Self::PPMapEntryHasNoCorrespondingProject(person, project) => {
                write!(f, "Person {person} is mapped to Project {project} but that project does not exist.")
            }
        }
    }
}
impl std::error::Error for DBError {}

/// Get a list of projects
pub async fn get_projects(pool: PgPool) -> Result<Vec<Project<FullId>>, DBError> {
    // first get all projects (required if projects are empty)
    let rows = sqlx::query!("SELECT ProjectID, ProjectName, ProjectRoomId FROM Project;")
        .fetch_all(&pool)
        .await
        .map_err(DBError::CannotSelectProjects)?;
    let mut result = rows
        .into_iter()
        .map(|r| Project::new((r.projectroomid, r.projectid), r.projectname))
        .collect::<Vec<Project<FullId>>>();

    // Now get all users part of any projects
    let rows = sqlx::query!(
        "SELECT Project.ProjectID, Project.ProjectName, Person.PersonID, Person.PersonName, Person.PersonSurname, Person.PersonFirstname, Person.IsGlobalAdmin, PersonProjectMap.IsProjectAdmin
            FROM Project
        INNER JOIN PersonProjectMap
            ON Project.ProjectID = PersonProjectMap.ProjectID
        INNER JOIN Person
            ON PersonProjectMap.PersonID = Person.PersonID;"
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

        for project in result.iter_mut() {
            if project.db_id() == row.projectid {
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
        warn!(
            "Person {}, Project {} is mapped but Project does not exist.",
            row.personid, row.projectid
        );
        return Err(DBError::PPMapEntryHasNoCorrespondingProject(
            row.personid,
            row.projectid,
        ));
    }
    Ok(result)
}

pub(crate) async fn add_project_prepare(
    pool: PgPool,
    project: Project<MatrixNoDb>,
) -> Result<(Transaction<'static, Postgres>, Project<FullId>), DBError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(DBError::CannotStartTransaction)?;

    let new_id = sqlx::query!(
        "INSERT INTO Project (ProjectName, ProjectRoomId) VALUES ($1, $2) RETURNING ProjectID;",
        project.name,
        project.matrix_id(),
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(DBError::CannotInsertProject)?;
    let idd_project = project.set_db_id(new_id.projectid);

    for member in idd_project.members.iter() {
        sqlx::query!("INSERT INTO PersonProjectMap (PersonID, ProjectID, IsProjectAdmin) VALUES ($1, $2, $3);",
            member.0.db_id(),
            new_id.projectid,
            member.1.is_admin(),
            )
            .execute(&mut *tx)
            .await
            .map_err(DBError::CannotInsertPPMap)?;
    }

    Ok((tx, idd_project))
}

/// Add a new project
#[allow(dead_code)]
pub(crate) async fn add_project(
    pool: PgPool,
    project: Project<MatrixNoDb>,
) -> Result<Project<FullId>, DBError> {
    let (tx, idd_project) = add_project_prepare(pool, project).await?;
    tx.commit()
        .await
        .map_err(DBError::CannotCommitTransaction)?;
    info!(
        "Created new project {} with {} users.",
        idd_project.name,
        idd_project.members.len()
    );
    Ok(idd_project)
}

/// Get a project from known ID
pub(crate) async fn get_project(
    con: &mut PgConnection,
    id: i32,
) -> Result<Option<Project<FullId>>, DBError> {
    // first get all projects (required if projects are empty)
    let rows = sqlx::query!(
        "SELECT ProjectID, ProjectName, ProjectRoomId FROM Project WHERE ProjectID = $1;",
        id,
    )
    .fetch_optional(&mut *con)
    .await
    .map_err(DBError::CannotSelectProjects)?;
    let mut project = match rows {
        None => {
            trace!("Project {id} does not exist.");
            return Ok(None);
        }
        Some(x) => Project::<FullId>::new((x.projectroomid, x.projectid), x.projectname),
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
        .fetch_all(con)
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

        if project.db_id() == row.projectid {
            project.add_member(
                person,
                UserPermission::new_from_is_admin(row.isprojectadmin),
            );
            continue 'row;
        };
        // no existing project fit - create a new one
        // This should not be possible because we have selected all projects in the same transaction
        warn!("Found no project for a PersonProjectMap entry. Check DB data integrity!");
        warn!(
            "Person {}, Project {} is mapped but Project does not exist.",
            row.personid, row.projectid
        );
        return Err(DBError::PPMapEntryHasNoCorrespondingProject(
            row.personid,
            row.projectid,
        ));
    }
    Ok(Some(project))
}

/// Remove a member from a project; Prepare a transcation, but do not commit it.
///
/// This is useful when we want to make commits dependent on another system also succeeding.
pub(crate) async fn remove_members_prepare<'a, 'b, 't>(
    pool: PgPool,
    project_id: i32,
    members_to_remove: &'a [&'b Person<DbNoMatrix>],
) -> Result<(i64, Transaction<'t, Postgres>), DBError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(DBError::CannotStartTransaction)?;
    let mut num_deleted = 0_i64;
    for mem in members_to_remove.iter() {
        let deleted = sqlx::query!(
            "WITH deleted AS (DELETE FROM PersonProjectMap WHERE PersonID = $1 AND ProjectID = $2 RETURNING *) SELECT count(*) from deleted;",
            mem.db_id(),
            project_id,
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(DBError::CannotRemoveMember)?;
        num_deleted += deleted.count.unwrap_or(0_i64);
    }

    Ok((num_deleted, tx))
}

/// remove the given persons from the given project
///
/// Internally calls [`remove_members_prepare`] which implements the logic.
#[allow(dead_code)]
pub(crate) async fn remove_members(
    pool: PgPool,
    project_id: i32,
    members_to_remove: &[&Person<DbNoMatrix>],
) -> Result<(), DBError> {
    let (num_deleted, tx) = remove_members_prepare(pool, project_id, members_to_remove).await?;

    tx.commit()
        .await
        .map_err(DBError::CannotCommitTransaction)?;
    trace!(
        "Deleted {} members from project {}.",
        num_deleted,
        project_id
    );
    Ok(())
}

async fn add_members_in_transaction(
    con: &mut PgConnection,
    project_id: i32,
    members_to_add: &[(&Person<DbNoMatrix>, &UserPermission)],
) -> Result<(), DBError> {
    for (mem, permission) in members_to_add.iter() {
        sqlx::query!(
            "INSERT INTO PersonProjectMap (PersonID, ProjectID, IsProjectAdmin) VALUES ($1, $2, $3);",
            mem.db_id(),
            project_id,
            permission.is_admin(),
        )
        .execute(&mut *con)
        .await
        .map_err(DBError::CannotInsertPPMap)?;
    }

    trace!(
        "Inserted {} new members to project {}.",
        members_to_add.len(),
        project_id
    );
    Ok(())
}

/// add the given persons to the project.
#[allow(dead_code)]
async fn add_members(
    pool: PgPool,
    project_id: i32,
    members_to_add: &[(&Person<DbNoMatrix>, &UserPermission)],
) -> Result<(), DBError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(DBError::CannotStartTransaction)?;
    add_members_in_transaction(&mut *tx, project_id, members_to_add).await?;

    tx.commit()
        .await
        .map_err(DBError::CannotCommitTransaction)?;
    trace!(
        "Inserted {} new members to project {}.",
        members_to_add.len(),
        project_id
    );
    Ok(())
}

pub(crate) async fn update_project_members_prepare<'a, 't>(
    pool: PgPool,
    project: &'a Project<FullId>,
) -> Result<Transaction<'t, Postgres>, DBError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(DBError::CannotStartTransaction)?;

    let old_project =
        get_project(&mut *tx, project.db_id())
            .await?
            .ok_or(DBError::ProjectDoesNotExist(
                project.db_id(),
                project.name.clone(),
            ))?;

    let members_to_remove = old_project
        .members
        .iter()
        .filter_map(|(m, _)| {
            if project.members.iter().all(|(n, _)| m != n) {
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
            if old_project.members.iter().all(|(n, _)| m != n) {
                Some((m, is_adm))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    // delete stale members
    let (_num_deleted, mut tx) =
        remove_members_prepare(pool.clone(), project.db_id(), &members_to_remove).await?;

    // add new members
    add_members_in_transaction(&mut *tx, project.db_id(), &members_to_add).await?;

    Ok(tx)
}

/// Set(overwrite) the members of a project
///
/// Internally calls [`update_project_members_prepare`] which implements the logic
#[allow(dead_code)]
pub(crate) async fn update_project_members(
    pool: PgPool,
    project: &Project<FullId>,
) -> Result<(), DBError> {
    let tx = update_project_members_prepare(pool, project).await?;
    tx.commit()
        .await
        .map_err(DBError::CannotCommitTransaction)?;

    Ok(())
}

/// Set(overwrite) the members of a project
pub(crate) async fn update_member_permission(
    pool: PgPool,
    project_id: i32,
    person_id: i32,
    new_permission: UserPermission,
) -> Result<(), DBError> {
    sqlx::query!(
        "UPDATE PersonProjectMap SET IsProjectAdmin = $1 WHERE PersonID = $2 AND ProjectID = $3;",
        new_permission.is_admin(),
        person_id,
        project_id,
    )
    .execute(&pool)
    .await
    .map_err(DBError::CannotUpdateMemberPermission)?;
    Ok(())
}

/// Change the name of a project in the db.
pub(crate) async fn rename_project_in_tx(
    con: &mut PgConnection,
    project_id: i32,
    new_name: &str,
) -> Result<(), DBError> {
    sqlx::query!(
        "UPDATE Project SET ProjectName = $1 WHERE ProjectId = $2;",
        new_name,
        project_id,
    )
    .execute(con)
    .await
    .map_err(DBError::CannotChangeProjectName)?;
    Ok(())
}

/// Add a person.
#[allow(dead_code)]
async fn add_person(pool: PgPool, person: Person<NoId>) -> Result<Person<DbNoMatrix>, DBError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(DBError::CannotStartTransaction)?;
    let new_id_result = sqlx::query!(
        "INSERT INTO Person (PersonName, PersonSurname, PersonFirstname, IsGlobalAdmin) VALUES ($1, $2, $3, $4) RETURNING PersonID",
        &person.name,
        person.surname,
        person.firstname,
        &person.is_global_admin(),
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(DBError::CannotInsertPerson)?;

    tx.commit()
        .await
        .map_err(DBError::CannotCommitTransaction)?;

    Ok(Person::<DbNoMatrix>::new(
        new_id_result.personid,
        person.name,
        person.global_permission,
        person.surname,
        person.firstname,
    ))
}

/// Get a person from the DB by name (exact)
pub async fn get_person(pool: PgPool, name: &str) -> Result<Option<Person<DbNoMatrix>>, DBError> {
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
        Some(x) => Ok(Some(Person::<DbNoMatrix>::new(
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
async fn get_all_persons(pool: PgPool) -> Result<Vec<Person<DbNoMatrix>>, DBError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(DBError::CannotStartTransaction)?;
    let res = sqlx::query!(
        "SELECT PersonID, PersonName, PersonSurname, PersonFirstname, IsGlobalAdmin from Person;",
    )
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
pub async fn update_users(pool: PgPool, users: Vec<Person<NoId>>) -> Result<(), DBError> {
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
        sqlx::query!("DELETE FROM Person WHERE PersonID = $1;", user.db_id(),)
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
            "SELECT PersonID, PersonSurname, PersonFirstname, IsGlobalAdmin from Person WHERE PersonName LIKE $1;",
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
                if row.personfirstname != user.firstname {
                    // update name
                    sqlx::query!(
                        "UPDATE Person SET PersonFirstname = $1 WHERE PersonName = $2;",
                        user.firstname,
                        user.name,
                    )
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DBError::CannotUpdateFirstname(e, user.name.to_owned()))?;
                    trace!("User {} Firstname set to: {:?}", user.name, user.firstname,);
                };
                if row.personsurname != user.surname {
                    // update name
                    sqlx::query!(
                        "UPDATE Person SET PersonSurname = $1 WHERE PersonName = $2;",
                        user.surname,
                        user.name,
                    )
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DBError::CannotUpdateSurname(e, user.name.to_owned()))?;
                    trace!("User {} Firstname set to: {:?}", user.name, user.firstname,);
                };
            }
        };
    }

    tx.commit()
        .await
        .map_err(DBError::CannotCommitTransaction)?;
    Ok(())
}

pub(crate) async fn get_persons_with_similar_name(
    pool: PgPool,
    name_like: &str,
) -> Result<Vec<Person<DbNoMatrix>>, DBError> {
    Ok(sqlx::query!(
        "SELECT PersonID, PersonName, PersonSurname, PersonFirstname, IsGlobalAdmin, similarity($1, concat(PersonSurname, ' ', PersonFirstname))
        FROM Person
        ORDER BY similarity DESC
        LIMIT 5;",
        name_like,
        )
    .fetch_all(&pool)
    .await
    .map_err(DBError::CannotSelectSimilarNames)?
    .into_iter()
    .map(|r| Person::new(
                r.personid,
                r.personname,
                UserPermission::new_from_is_admin(r.isglobaladmin),
                r.personsurname,
                r.personfirstname,)
        )
    .collect::<Vec<_>>())
}

/// Test at runtime whether we can establish a connection to the DB
pub(crate) async fn try_acquire_connection(pool: PgPool) -> Result<(), DBError> {
    pool.begin()
        .await
        .map_err(DBError::CannotStartTransaction)?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use sqlx::PgPool;

    #[sqlx::test(fixtures("empty"))]
    async fn test_add_person(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let person = Person::<NoId> {
            person_id: NoId::default(),
            name: "John Doe".to_owned(),
            global_permission: UserPermission::User,
            firstname: Some("John".to_owned()),
            surname: Some("Doe".to_owned()),
        };
        add_person(pool.clone(), person).await.unwrap();
        let ps = get_all_persons(pool.clone()).await.unwrap();
        assert_eq!(
            ps.into_iter().next().unwrap(),
            Person::<DbNoMatrix>::new(
                1,
                "John Doe".to_owned(),
                UserPermission::User,
                Some("Doe".to_owned()),
                Some("John".to_owned())
            )
        );
        Ok(())
    }

    #[sqlx::test(fixtures("two_projects"))]
    async fn test_get_projects(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let projects = get_projects(pool.clone()).await.unwrap();
        assert_eq!(projects.len(), 2);
        Ok(())
    }

    #[sqlx::test(fixtures("empty_project"))]
    async fn test_get_projects_no_users(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let projects = get_projects(pool.clone()).await.unwrap();
        assert_eq!(projects.len(), 1);
        Ok(())
    }

    #[sqlx::test(fixtures("two_projects"))]
    async fn test_add_project(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let new_project: Project<MatrixNoDb> = Project::new("matrix-id".to_owned(), "some new name".to_owned());
        let idd_project = add_project(pool, new_project).await.unwrap();
        assert_eq!(idd_project.db_id(), 3);
        Ok(())
    }

    #[sqlx::test(fixtures("two_projects"))]
    async fn test_add_members(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let project_id = 1;
        let adam = Person::new(
            1,
            "Adam".to_owned(),
            UserPermission::Admin,
            Some("Adam".to_owned()),
            Some("Abrahamovitch".to_owned()),
        );
        let persons = vec![(&adam, &UserPermission::User)];
        add_members(pool.clone(), project_id, &persons).await?;

        let project = get_project(&mut pool.clone().acquire().await.unwrap(), project_id)
            .await?
            .unwrap();
        assert_eq!(project.members.len(), 3);
        Ok(())
    }

    #[sqlx::test(fixtures("two_projects"))]
    async fn test_remove_members(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let project_id = 1;
        let adam = Person::new(
            1,
            "Adam".to_owned(),
            UserPermission::Admin,
            Some("Adam".to_owned()),
            Some("Abrahamovitch".to_owned()),
        );
        let persons = vec![&adam];
        remove_members(pool.clone(), project_id, &persons).await?;

        let project = get_project(&mut pool.clone().acquire().await.unwrap(), project_id)
            .await?
            .unwrap();
        assert_eq!(project.members.len(), 1);
        Ok(())
    }

    #[sqlx::test(fixtures("two_projects"))]
    async fn test_update_members(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let project_id = 1;
        let david = Person::<NoId>::new(
            (),
            "David".to_owned(),
            UserPermission::Admin,
            Some("David".to_owned()),
            Some("Descartes".to_owned()),
        );
        let hanna = Person::<NoId>::new(
            (),
            "Hanna".to_owned(),
            UserPermission::User,
            Some("Hanna".to_owned()),
            Some("Herakleia".to_owned()),
        );
        let samuel = Person::<NoId>::new(
            (),
            "Samuel".to_owned(),
            UserPermission::User,
            Some("Samuel".to_owned()),
            Some("Shmuelov".to_owned()),
        );

        let david = add_person(pool.clone(), david).await?;
        let hanna = add_person(pool.clone(), hanna).await?;
        let samuel = add_person(pool.clone(), samuel).await?;

        let mut basil_1 = Project::new(("matrix-id".to_owned(), 1), "1Basil".to_owned());
        basil_1.members.push((david, UserPermission::User));
        basil_1.members.push((hanna, UserPermission::Admin));
        basil_1.members.push((samuel, UserPermission::Admin));

        update_project_members(pool.clone(), &basil_1).await?;

        let project = get_project(&mut pool.clone().acquire().await.unwrap(), project_id)
            .await?
            .unwrap();
        assert_eq!(project.members.len(), 3);

        Ok(())
    }

    #[sqlx::test(fixtures("two_projects"))]
    async fn test_update_users(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let david = Person::<NoId>::new(
            (),
            "David".to_owned(),
            UserPermission::Admin,
            Some("David".to_owned()),
            Some("Descartes".to_owned()),
        );
        let hanna = Person::<NoId>::new(
            (),
            "Hanna".to_owned(),
            UserPermission::User,
            Some("Hanna".to_owned()),
            Some("Herakleia".to_owned()),
        );
        let samuel = Person::<NoId>::new(
            (),
            "Samuel".to_owned(),
            UserPermission::User,
            Some("Samuel".to_owned()),
            Some("Shmuelov".to_owned()),
        );

        update_users(pool.clone(), vec![david, hanna, samuel]).await?;
        let persons = get_all_persons(pool.clone()).await?;
        assert_eq!(persons.len(), 3);

        Ok(())
    }

    #[sqlx::test(fixtures("two_projects"))]
    async fn test_update_member_permission(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        update_member_permission(pool.clone(), 1, 1, UserPermission::User).await?;

        let res = get_project(&mut pool.clone().acquire().await.unwrap(), 1)
            .await?
            .unwrap();
        for (member, perm) in res.members {
            if member.db_id() == 1 {
                assert_eq!(perm, UserPermission::User);
            };
        }
        Ok(())
    }
}
