//! The [`Project`] type used throughout

use core::borrow::Borrow;

use askama::{Html, Template};

use super::{HasID, NoID, Person, UserPermission, DBID};

#[derive(Debug)]
pub(crate) struct Project<I: DBID> {
    project_id: I,
    pub(crate) name: String,
    pub(crate) members: Vec<(Person<HasID>, UserPermission)>,
}

#[derive(Template)]
#[template(path = "project/header_only.html")]
pub(crate) struct ProjectTemplate<'a> {
    project: &'a Project<HasID>,
    permission: UserPermission,
}
impl<I> Project<I>
where
    I: DBID,
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
}

#[derive(askama::Template)]
#[template(path = "project/header_only.html")]
struct ProjectDisplayHeaderOnly<'a> {
    project: &'a Project<HasID>,
}

#[derive(askama::Template)]
#[template(path = "project/with_users.html")]
struct ProjectDisplayWithUsers<'a> {
    project: &'a Project<HasID>,
    permission: UserPermission,
}

impl Project<HasID> {
    pub(crate) fn project_id(&self) -> i32 {
        self.project_id.id
    }

    pub(crate) fn add_member(&mut self, person: Person<HasID>, permission: UserPermission) {
        self.members.push((person, permission));
    }

    /// Render self, displaying only the header
    pub(crate) fn display_header_only(&self) -> String {
        ProjectDisplayHeaderOnly { project: self }
            .render()
            .expect("static template")
    }

    /// Render self, displaying only the header
    pub(crate) fn display_with_users(&self, permission: UserPermission) -> String {
        ProjectDisplayWithUsers {
            project: self,
            permission,
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
        person: &Person<HasID>,
    ) -> Option<UserPermission> {
        for (user, perm) in self.members.iter() {
            if user.person_id == person.person_id {
                return Some(*perm);
            }
        }
        None
    }
}

impl Project<NoID> {
    pub(crate) fn set_id<I: Into<HasID>>(self, id: I) -> Project<HasID> {
        Project {
            project_id: id.into(),
            name: self.name,
            members: self.members,
        }
    }
}
