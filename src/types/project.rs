//! The [`Project`] type used throughout

use core::borrow::Borrow;
use std::{borrow::Cow, sync::Arc};

use askama::{Html, Template};

use crate::config::Config;

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
    /// permission of the user requesting the template
    view_permission: UserPermission,
    matrix_server: String,
    element_server: String,
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

    pub fn matrix_room_alias_local(&self) -> Cow<'_, str> {
        urlencoding::encode(&self.name)
    }
}

#[derive(askama::Template)]
#[template(path = "project/header_only.html")]
struct ProjectDisplayHeaderOnly<'a> {
    project: &'a Project<HasID>,
    element_server: String,
    matrix_server: String,
}

#[derive(askama::Template)]
#[template(path = "project/with_users.html", escape = "none")]
struct ProjectDisplayWithUsers<'a> {
    project: &'a Project<HasID>,
    /// Permission of the person requesting the template
    view_permission: UserPermission,
    element_server: String,
    matrix_server: String,
}

impl Project<HasID> {
    pub(crate) fn project_id(&self) -> i32 {
        self.project_id.id
    }

    pub(crate) fn add_member(&mut self, person: Person<HasID>, permission: UserPermission) {
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
