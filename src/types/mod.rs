//! Basic types used throughout the program.

mod user_permission;
pub(crate) use user_permission::UserPermission;

mod person;
pub(crate) use person::Person;

mod project;
pub(crate) use project::Project;

pub(crate) trait IdState: core::fmt::Debug {
    type DbId: Copy;
    type MatrixId;

    fn db_id(&self) -> &Self::DbId;
    fn matrix_id(&self) -> &Self::MatrixId;
}

#[derive(Debug, Eq, PartialEq, Clone, Copy, Default)]
pub struct NoId {}
impl IdState for NoId {
    type DbId = ();
    type MatrixId = ();

    fn db_id(&self) -> &Self::DbId {
        &()
    }
    fn matrix_id(&self) -> &Self::MatrixId {
        &()
    }
}
impl From<()> for NoId {
    fn from(_value: ()) -> Self {
        Self {}
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct MatrixNoDb {
    matrix_id: String,
}
impl IdState for MatrixNoDb {
    type DbId = ();
    type MatrixId = String;
    fn db_id(&self) -> &Self::DbId {
        &()
    }
    fn matrix_id(&self) -> &Self::MatrixId {
        &self.matrix_id
    }
}
impl From<String> for MatrixNoDb {
    fn from(value: String) -> Self {
        Self { matrix_id: value }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct DbNoMatrix {
    db_id: i32,
}
impl IdState for DbNoMatrix {
    type DbId = i32;
    type MatrixId = ();
    fn db_id(&self) -> &Self::DbId {
        &self.db_id
    }
    fn matrix_id(&self) -> &Self::MatrixId {
        &()
    }
}
impl From<i32> for DbNoMatrix {
    fn from(value: i32) -> Self {
        Self { db_id: value }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct FullId {
    db_id: i32,
    matrix_id: String,
}
impl IdState for FullId {
    type DbId = i32;
    type MatrixId = String;
    fn db_id(&self) -> &Self::DbId {
        &self.db_id
    }
    fn matrix_id(&self) -> &Self::MatrixId {
        &self.matrix_id
    }
}
impl From<(String, i32)> for FullId {
    fn from(value: (String, i32)) -> Self {
        Self {
            matrix_id: value.0,
            db_id: value.1,
        }
    }
}
