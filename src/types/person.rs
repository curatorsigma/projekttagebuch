//! The [`Person`] Type used throughout


use askama::Template;

use super::{DbNoMatrix, IdState, NoId, UserPermission};

// todo have "view-perm" and person-status in this project separately
#[derive(askama::Template)]
#[template(path = "user/show.html")]
struct UserTemplate<'a> {
    person: &'a Person<DbNoMatrix>,
    /// the project this user is shown in
    project_id: i32,
    /// The permission of the users viewing this template
    ///
    /// This decides wheter `remove user` and `promote/demote user` is shown.
    view_permission: UserPermission,
    /// The permission of this user in its group
    ///
    /// This determins whether they are shown as `Admin` or `User`
    local_permission: UserPermission,
}


pub(crate) trait PersonIdState: IdState {}
impl PersonIdState for NoId {}
impl PersonIdState for DbNoMatrix {}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) struct Person<I: PersonIdState> {
    pub(crate) person_id: I,
    pub(crate) name: String,
    pub(crate) global_permission: UserPermission,
    pub(crate) surname: Option<String>,
    pub(crate) firstname: Option<String>,
}
impl<I> Person<I>
where
    I: PersonIdState,
{
    pub fn new<IdInto>(
        person_id: IdInto,
        name: String,
        global_permission: UserPermission,
        surname: Option<String>,
        firstname: Option<String>,
    ) -> Self
    where
        IdInto: Into<I>,
    {
        Self {
            person_id: person_id.into(),
            name,
            global_permission,
            surname,
            firstname,
        }
    }

    pub fn is_global_admin(&self) -> bool {
        match self.global_permission {
            UserPermission::Admin => true,
            UserPermission::User => false,
        }
    }

    pub fn db_id(&self) -> I::DbId {
        *self.person_id.db_id()
    }
}

impl Person<DbNoMatrix> {
    /// template the user-line for this user
    pub fn display<A>(&self, project_id: i32, view_permission: A, local_permission: A) -> String
    where
        A: AsRef<UserPermission>,
    {
        UserTemplate {
            person: self,
            project_id,
            // this is a bit of weird magic - askama templates take these permission by-ref
            // (because they are in for-loops which .iter() )
            // But we want to pass it as owned
            local_permission: local_permission.as_ref().to_owned(),
            view_permission: view_permission.as_ref().to_owned(),
        }
        .render()
        .expect("static template")
    }
}
