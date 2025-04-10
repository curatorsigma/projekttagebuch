//! The [`Project`] type used throughout

use askama::Template;

use super::{DbNoMatrix, FullId, IdState, MatrixNoDb, NoId, Person, UserPermission};

/// These are the possible states a projects ID can be in
pub(crate) trait ProjectIdState: IdState {}
impl ProjectIdState for NoId {}
impl ProjectIdState for MatrixNoDb {}
impl ProjectIdState for FullId {}


#[derive(Debug)]
pub(crate) struct Project<I: ProjectIdState> {
    project_id: I,
    pub(crate) name: String,
    pub(crate) members: Vec<(Person<DbNoMatrix>, UserPermission)>,
}

impl<I> Project<I>
where
    I: ProjectIdState,
{
    pub fn new<IdInto>(project_id: IdInto, name: String) -> Self
    where
        IdInto: Into<I>,
    {
        Self {
            project_id: project_id.into(),
            name,
            members: vec![],
        }
    }

    pub fn db_id(&self) -> I::DbId {
        *self.project_id.db_id()
    }

    pub fn matrix_id(&self) -> &I::MatrixId {
        self.project_id.matrix_id()
    }
}

#[derive(askama::Template)]
#[template(path = "project/header_only.html")]
struct ProjectDisplayHeaderOnly<'a> {
    project: &'a Project<FullId>,
    element_server: String,
    matrix_server: String,
}

#[derive(askama::Template)]
#[template(path = "project/with_users.html", escape = "none")]
struct ProjectDisplayWithUsers<'a> {
    project: &'a Project<FullId>,
    /// Permission of the person requesting the template
    view_permission: UserPermission,
    element_server: String,
    matrix_server: String,
}

impl Project<FullId> {
    pub(crate) fn add_member(&mut self, person: Person<DbNoMatrix>, permission: UserPermission) {
        self.members.push((person, permission));
    }

    /// Render self, displaying only the header
    pub(crate) fn display_header_only(
        &self,
        matrix_server: String,
        element_server: String,
    ) -> String {
        ProjectDisplayHeaderOnly {
            project: self,
            element_server,
            matrix_server,
        }
        .render()
        .expect("static template")
    }

    /// Render self, displaying only the header
    pub(crate) fn display_with_users(
        &self,
        view_permission: UserPermission,
        matrix_server: String,
        element_server: String,
    ) -> String {
        ProjectDisplayWithUsers {
            project: self,
            view_permission,
            element_server,
            matrix_server,
        }
        .render()
        .expect("static template")
    }

    /// None, when the user is not in the group.
    /// Some(Admin) when they have admin privileges for this group
    /// Some(User) when they have normal privileges for this group
    ///
    /// IGNORES global permissions for the user
    pub(crate) fn local_permission_for_user(
        &self,
        person: &Person<DbNoMatrix>,
    ) -> Option<UserPermission> {
        for (user, perm) in self.members.iter() {
            if user.person_id == person.person_id {
                return Some(*perm);
            }
        }
        None
    }
}

impl Project<NoId> {
    pub(crate) fn set_matrix_id<I: Into<MatrixNoDb>>(self, id: I) -> Project<MatrixNoDb> {
        Project {
            project_id: id.into(),
            name: self.name,
            members: self.members,
        }
    }
}
impl Project<MatrixNoDb> {
    pub(crate) fn set_db_id<I: Into<<FullId as IdState>::DbId>>(self, id: I) -> Project<FullId> {
        Project {
            project_id: FullId { db_id: id.into(), matrix_id: self.project_id.matrix_id}, name: self.name, members: self.members, 
        }
    }
}
