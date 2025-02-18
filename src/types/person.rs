//! The [`Person`] Type used throughout

use super::DBID;

#[derive(Debug)]
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
