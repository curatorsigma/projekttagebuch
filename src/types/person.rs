//! The [`Person`] Type used throughout

use core::borrow::Borrow;

use askama::Template;

use super::{HasID, UserPermission, DBID};


// todo have "view-perm" and person-status in this project separately
#[derive(askama::Template)]
#[template(path = "user/show.html")]
struct UserTemplate<'a> {
    person: &'a Person<HasID>,
    perm: UserPermission,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) struct Person<I: DBID> {
    pub(crate) person_id: I,
    pub(crate) name: String,
    pub(crate) global_permission: UserPermission,
    pub(crate) surname: Option<String>,
    pub(crate) firstname: Option<String>,
}
impl<I> Person<I>
where
    I: DBID,
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
}

impl Person<HasID> {
    pub fn person_id(&self) -> i32 {
        self.person_id.id
    }

    /// template the user-line for this user
    pub fn display(&self, perm: UserPermission) -> String {
        UserTemplate {
            person: self,
            perm,
        }.render().expect("static template")
    }
}
