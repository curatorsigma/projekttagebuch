//! Basic types used throughout the program.

mod user_permission;
pub(crate) use user_permission::UserPermission;

mod person;
pub(crate) use person::Person;

mod project;
pub(crate) use project::Project;

trait DBID: core::fmt::Debug {}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub(crate) struct HasID {
    pub(crate) id: i32,
}
impl DBID for HasID {}
impl From<i32> for HasID {
    fn from(value: i32) -> Self {
        Self { id: value }
    }
}
impl HasID {
    pub fn new(id: i32) -> Self {
        Self { id }
    }
}

#[derive(Debug)]
pub(crate) struct NoID {}
impl DBID for NoID {}
impl From<()> for NoID {
    fn from(value: ()) -> Self {
        Self {}
    }
}
impl Default for NoID {
    fn default() -> Self {
        Self {  }
    }
}

