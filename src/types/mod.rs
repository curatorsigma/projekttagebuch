//! Basic types used throughout the program.

mod user_permission;
pub(crate) use user_permission::UserPermission;

mod person;
pub(crate) use person::Person;

mod project;
pub(crate) use project::Project;

pub(crate) trait DbId: core::fmt::Debug {}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub(crate) struct HasID {
    pub(crate) id: i32,
}
impl DbId for HasID {}
impl From<i32> for HasID {
    fn from(value: i32) -> Self {
        Self { id: value }
    }
}

#[derive(Debug, Default)]
pub(crate) struct NoID {}
impl DbId for NoID {}
impl From<()> for NoID {
    fn from(_value: ()) -> Self {
        Self {}
    }
}
