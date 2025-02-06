//! The [`Project`] type used throughout

use super::{HasID, Person, DBID};

#[derive(Debug)]
pub(crate) struct Project<I : DBID> {
    project_id: I,
    name: String,
    members: Vec<Person<HasID>>,
}
