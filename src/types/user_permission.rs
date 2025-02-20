//! Permissions that an individual user can have.

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum UserPermission {
    User,
    Admin,
}
impl UserPermission {
    pub fn new_from_is_admin(is_admin: bool) -> Self {
        if is_admin {
            UserPermission::Admin
        } else {
            UserPermission::User
        }
    }

    pub fn is_admin(&self) -> bool {
        match self {
            Self::User => false,
            Self::Admin => true,
        }
    }
}
