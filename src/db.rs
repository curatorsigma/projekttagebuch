//! Low level Database primitives

use sqlx::{postgres::PgRow, PgPool, Postgres, Row};

use crate::types::{HasID, NoID, Person, Project};

#[derive(Debug)]
pub(crate) enum DBError {
    CannotAuthenticate(sqlx::Error),
    CannotStartTransaction(sqlx::Error),
    CannotCommitTransaction(sqlx::Error),
    CannotRollbackTransaction(sqlx::Error),
    CannotInsertPerson(sqlx::Error),
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
        }
    }
}
impl std::error::Error for DBError {}

/// Get a list of projects
async fn get_projects(pool: PgPool) -> Result<Vec<Project<HasID>>, DBError> {
    // get projects
    // get members from the mapping
    todo!()
}

/// Add a new project
async fn add_project(project: Project<NoID>) -> Result<Project<HasID>, DBError> {
    // insert the persons
    // insert the project
    // insert the person mappings
    // (do all this in a transaction, rollback on error)
    todo!()
}

/// Set(overwrite) the members of a project
async fn update_project_members(project: Project<HasID>) -> Result<(), DBError> {
    // read current members
    // delete stale members
    // add new members
    todo!()
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

/// Get a person from the DB by name
async fn get_person(name: &str) -> Result<Option<Person<HasID>>, DBError> {
    todo!()
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
}
