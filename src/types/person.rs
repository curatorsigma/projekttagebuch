//! The [`Person`] Type used throughout

use super::{HasID, DBID};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Person<I: DBID> {
    pub(crate) person_id: I,
    pub(crate) name: String,
}
impl<I> Person<I>
where
    I: DBID,
{
    pub fn new<IdInto>(person_id: IdInto, name: String) -> Self
    where
        IdInto: Into<I>,
    {
        Self {
            person_id: person_id.into(),
            name,
        }
    }
}

impl Person<HasID> {
    pub fn person_id(&self) -> i32 {
        self.person_id.id
    }
}
