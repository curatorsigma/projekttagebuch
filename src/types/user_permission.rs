//! Permissions that an individual user can have.

#[derive(Debug, Copy, Clone)]
pub(crate) enum UserPermission {
    User,
    Admin,
}
