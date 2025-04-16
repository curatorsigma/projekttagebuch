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
#[template(path = "project/header_only.html", escape="none")]
struct ProjectDisplayHeaderOnly<'a> {
    project: &'a Project<FullId>,
    view_permission: UserPermission,
    element_server: String,
}

#[derive(askama::Template)]
#[template(path = "project/with_users.html", escape = "none")]
struct ProjectDisplayWithUsers<'a> {
    project: &'a Project<FullId>,
    /// Permission of the person requesting the template
    view_permission: UserPermission,
    element_server: String,
}

#[derive(askama::Template)]
#[template(path = "project/name_show.html")]
struct ProjectNameDisplay<'a> {
    project: &'a Project<FullId>,
    /// Permission of the person requesting the template
    view_permission: UserPermission,
}


impl Project<FullId> {
    pub(crate) fn add_member(&mut self, person: Person<DbNoMatrix>, permission: UserPermission) {
        self.members.push((person, permission));
    }

    /// Render self, displaying only the header
    pub(crate) fn display_header_only(
        &self,
        user: &Person<DbNoMatrix>,
        element_server: String,
    ) -> String {
        let view_permission = UserPermission::new_from_is_admin(
            user.is_global_admin() || self.local_permission_for_user(&user).is_some_and(|x| x.is_admin()));
        ProjectDisplayHeaderOnly {
            project: self,
            view_permission,
            element_server,
        }
        .render()
        .expect("static template")
    }

    /// Render self, displaying only the header
    pub(crate) fn display_with_users(
        &self,
        view_permission: UserPermission,
        element_server: String,
    ) -> String {
        ProjectDisplayWithUsers {
            project: self,
            view_permission,
            element_server,
        }
        .render()
        .expect("static template")
    }

    /// Render self.name with the edit button next to it if the user has permission
    pub(crate) fn display_name(
        &self,
        view_permission: &UserPermission,
    ) -> String {
        ProjectNameDisplay {
            project: self,
            view_permission: *view_permission,
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
            project_id: FullId {
                db_id: id.into(),
                matrix_id: self.project_id.matrix_id,
            },
            name: self.name,
            members: self.members,
        }
    }
}
