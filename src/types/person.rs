//! The [`Person`] Type used throughout

use super::{HasID, UserPermission, DBID};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Person<I: DBID> {
    pub(crate) person_id: I,
    pub(crate) name: String,
    pub(crate) global_permission: UserPermission,
    pub(crate) surname: String,
    pub(crate) firstname: String,
}
impl<I> Person<I>
where
    I: DBID,
{
    pub fn new<IdInto>(person_id: IdInto, name: String, global_permission: UserPermission, surname: String, firstname: String) -> Self
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
}
