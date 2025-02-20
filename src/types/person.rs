//! The [`Person`] Type used throughout

use super::{HasID, UserPermission, DBID};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Person<I: DBID> {
    pub(crate) person_id: I,
    pub(crate) name: String,
    pub(crate) global_permission: UserPermission,
}
impl<I> Person<I>
where
    I: DBID,
{
    pub fn new<IdInto>(person_id: IdInto, name: String, global_permission: UserPermission) -> Self
    where
        IdInto: Into<I>,
    {
        Self {
            person_id: person_id.into(),
            name,
            global_permission,
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
