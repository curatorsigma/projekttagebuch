//! The [`Project`] type used throughout

use askama::Template;

use super::{HasID, Person, UserPermission, DBID};

#[derive(Debug)]
pub(crate) struct Project<I: DBID> {
    project_id: I,
    name: String,
    members: Vec<Person<HasID>>,
}

#[derive(Template)]
#[template(path = "project/header_only.html")]
pub(crate) struct ProjectTemplate<'a> {
    project: &'a Project<HasID>,
    permission: UserPermission,
}

impl Project<HasID> {
    pub(crate) fn to_template(&self, permission: &UserPermission) -> ProjectTemplate {
        ProjectTemplate { project: self, permission: *permission, }
    }
}
