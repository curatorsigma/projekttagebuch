//! Used as the backbone for "actions that change server state"
//! This module does not deal with routing and extracting form data or writing http responses
//!
//! Instead, both web and api frontends may call these actions once they have extracted the
//! necessary information from the user-supplied data and may then prepare the correct response
//! themselves.

use std::sync::Arc;

use tracing::{debug, info};

use crate::{
    config::Config,
    db::{
        add_project_prepare, get_person, get_project,
        remove_members_prepare, update_member_permission,
        update_project_members_prepare, DBError,
    },
    matrix::MatrixClientError,
    types::{HasID, NoID, Person, Project, UserPermission},
};

#[derive(Debug)]
pub(super) enum AddMemberError {
    ProjectDoesNotExist,
    /// Name of the Project the requester wanted to add to
    /// (the caller does not know how that project is called yet)
    RequesterHasNoPermission(String),
    PersonDoesNotExist,
    DB(DBError),
    Matrix(MatrixClientError),
}
impl core::fmt::Display for AddMemberError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProjectDoesNotExist => {
                write!(f, "The project does not exist.")
            }
            Self::PersonDoesNotExist => {
                write!(f, "The person does not exist.")
            }
            Self::RequesterHasNoPermission(_) => {
                write!(f, "The requester does not have the necessary permissions.")
            }
            Self::DB(e) => {
                write!(f, "The DB returned this error: {e}.")
            }
            Self::Matrix(e) => {
                write!(f, "Error communicating with matrix server: {e}")
            }
        }
    }
}
impl std::error::Error for AddMemberError {}
impl From<MatrixClientError> for AddMemberError {
    fn from(value: MatrixClientError) -> Self {
        Self::Matrix(value)
    }
}

/// Add a new member to a group and make sure all state is ok.
///
/// This function also checks permission of the requester.
///
/// Return:
/// - the person that was aded to the project
/// - the project the person was added to
/// Or the appropriate error
pub(super) async fn add_member_to_project(
    config: Arc<Config>,
    requester: &Person<HasID>,
    new_member_name: &str,
    project_id: i32,
) -> Result<(Person<HasID>, Project<HasID>), AddMemberError> {
    // the permission to do this depends on the project, so we need to get that before checking
    // permission
    let mut con = config
        .pg_pool
        .clone()
        .acquire()
        .await
        .map_err(|e| AddMemberError::DB(DBError::CannotStartTransaction(e)))?;
    let mut project = match get_project(&mut con, project_id).await {
        Ok(Some(x)) => x,
        Ok(None) => {
            return Err(AddMemberError::ProjectDoesNotExist);
        }
        Err(e) => {
            return Err(AddMemberError::DB(e));
        }
    };

    let user_may_add_member_to_this_group = match project.local_permission_for_user(&requester) {
        Some(UserPermission::Admin) => true,
        Some(UserPermission::User) => requester.is_global_admin(),
        None => requester.is_global_admin(),
    };
    if !user_may_add_member_to_this_group {
        return Err(AddMemberError::RequesterHasNoPermission(project.name));
    };

    // The user is allowed to add members to project.
    // Now we need to make sure the new member is actually a known user.
    let new_member = match get_person(config.pg_pool.clone(), new_member_name).await {
        Ok(Some(x)) => x,
        Ok(None) => {
            return Err(AddMemberError::PersonDoesNotExist);
        }
        Err(e) => {
            return Err(AddMemberError::DB(e));
        }
    };

    // Everything okay. Add the new member.
    project.add_member(new_member.clone(), UserPermission::User);

    match update_project_members_prepare(config.pg_pool.clone(), &project).await {
        Ok(tx) => {
            debug!(
                "Prepared a transaction to add {} to {}. Now trying to add to Matrix...",
                new_member.name, project.name
            );
            // now try to make the deletion from Matrix
            let mut our_client = config.matrix_client.clone();
            our_client
                .ensure_user_in_room(&new_member, &project)
                .await?;
            debug!("Successfully added {} to {} in Matrix. Now trying to commit the held DB transaction...", new_member.name, project.name);
            tx.commit()
                .await
                .map_err(|e| AddMemberError::DB(DBError::CannotCommitTransaction(e)))?;

            info!(
                "Added {} to {} as User; request made by {}.",
                new_member.name, project.name, requester.name
            );
            Ok((new_member, project))
        }
        Err(e) => Err(AddMemberError::DB(e)),
    }
}

#[derive(Debug)]
pub(super) enum RemoveMemberError {
    ProjectDoesNotExist,
    /// Name of the Project the requester wanted to add to
    /// (the caller does not know how that project is called yet)
    RequesterHasNoPermission(String),
    PersonDoesNotExist,
    DB(DBError),
    Matrix(MatrixClientError),
}
impl core::fmt::Display for RemoveMemberError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProjectDoesNotExist => {
                write!(f, "The project does not exist.")
            }
            Self::PersonDoesNotExist => {
                write!(f, "The person does not exist.")
            }
            Self::RequesterHasNoPermission(_) => {
                write!(f, "The requester does not have the necessary permissions.")
            }
            Self::DB(e) => {
                write!(f, "The DB returned this error: {e}.")
            }
            Self::Matrix(e) => {
                write!(f, "Error communicating with matrix server: {e}")
            }
        }
    }
}
impl std::error::Error for RemoveMemberError {}
impl From<MatrixClientError> for RemoveMemberError {
    fn from(value: MatrixClientError) -> Self {
        Self::Matrix(value)
    }
}

/// Remove a member from a group and make sure all state is ok.
///
/// This function also checks permission of the requester.
///
/// Return:
/// - the person that was removed from the project
/// - the project the person was removed from
/// Or the appropriate error
pub(super) async fn remove_member_from_project(
    config: Arc<Config>,
    requester: &Person<HasID>,
    remove_member_name: &str,
    project_id: i32,
) -> Result<(Person<HasID>, Project<HasID>), RemoveMemberError> {
    // the permission to do this depends on the project, so we need to get that before checking
    // permission
    let mut con = config
        .pg_pool
        .clone()
        .acquire()
        .await
        .map_err(|e| RemoveMemberError::DB(DBError::CannotStartTransaction(e)))?;
    let project = match get_project(&mut con, project_id).await {
        Ok(Some(x)) => x,
        Ok(None) => {
            return Err(RemoveMemberError::ProjectDoesNotExist);
        }
        Err(e) => {
            return Err(RemoveMemberError::DB(e));
        }
    };

    let user_may_remove_member_from_this_group = match project.local_permission_for_user(&requester)
    {
        Some(UserPermission::Admin) => true,
        Some(UserPermission::User) => requester.is_global_admin(),
        None => requester.is_global_admin(),
    };
    if !user_may_remove_member_from_this_group {
        return Err(RemoveMemberError::RequesterHasNoPermission(project.name));
    };

    // The user is allowed to remove members to project.
    // Now we need to make sure the remove member is actually a known user.
    let remove_member = match get_person(config.pg_pool.clone(), remove_member_name).await {
        Ok(Some(x)) => x,
        Ok(None) => {
            return Err(RemoveMemberError::PersonDoesNotExist);
        }
        Err(e) => {
            return Err(RemoveMemberError::DB(e));
        }
    };

    match remove_members_prepare(
        config.pg_pool.clone(),
        project.project_id(),
        &[&remove_member],
    )
    .await
    {
        Ok((_num_deleted, tx)) => {
            debug!(
                "Prepared a transaction to remove {} from {}. Now trying to remove from Matrix...",
                remove_member.name, project.name
            );
            // now try to make the deletion from Matrix
            let mut our_client = config.matrix_client.clone();
            our_client
                .ensure_user_not_in_room(&remove_member, &project)
                .await?;
            debug!("Successfully removed {} from {} in Matrix. Now trying to commit the held DB transaction...", remove_member.name, project.name);
            tx.commit()
                .await
                .map_err(|e| RemoveMemberError::DB(DBError::CannotCommitTransaction(e)))?;

            // both matrix and DB have agreed that all is well
            info!(
                "Removed {} from {} as User; request made by {}.",
                remove_member.name, project.name, requester.name
            );
            Ok((remove_member, project))
        }
        Err(e) => Err(RemoveMemberError::DB(e)),
    }
}

#[derive(Debug)]
pub(super) enum SetPermissionError {
    ProjectDoesNotExist,
    /// Name of the Project the requester wanted to add to
    /// (the caller does not know how that project is called yet)
    RequesterHasNoPermission(String),
    PersonDoesNotExist,
    DB(DBError),
}
impl core::fmt::Display for SetPermissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProjectDoesNotExist => {
                write!(f, "The project does not exist.")
            }
            Self::PersonDoesNotExist => {
                write!(f, "The person does not exist.")
            }
            Self::RequesterHasNoPermission(_) => {
                write!(f, "The requester does not have the necessary permissions.")
            }
            Self::DB(e) => {
                write!(f, "The DB returned this error: {e}.")
            }
        }
    }
}
impl std::error::Error for SetPermissionError {}

pub async fn set_member_permission(
    config: Arc<Config>,
    requester: &Person<HasID>,
    change_member_name: &str,
    project_id: i32,
    new_permission: UserPermission,
) -> Result<(Person<HasID>, Project<HasID>), SetPermissionError> {
    // the permission to do this depends on the project, so we need to get that before checking
    // permission
    let mut con = config
        .pg_pool
        .clone()
        .acquire()
        .await
        .map_err(|e| SetPermissionError::DB(DBError::CannotStartTransaction(e)))?;
    let project = match get_project(&mut con, project_id).await {
        Ok(Some(x)) => x,
        Ok(None) => {
            return Err(SetPermissionError::ProjectDoesNotExist);
        }
        Err(e) => {
            return Err(SetPermissionError::DB(e));
        }
    };

    let user_may_set_member_permissions = match project.local_permission_for_user(&requester) {
        Some(UserPermission::Admin) => true,
        Some(UserPermission::User) => requester.is_global_admin(),
        None => requester.is_global_admin(),
    };
    if !user_may_set_member_permissions {
        return Err(SetPermissionError::RequesterHasNoPermission(
            project.name.clone(),
        ));
    };

    // The user is allowed to set member permissions on this project.
    // Now we need to make sure the requested member is actually a known user.
    let change_member = match get_person(config.pg_pool.clone(), &change_member_name).await {
        Ok(Some(x)) => x,
        Ok(None) => {
            return Err(SetPermissionError::PersonDoesNotExist);
        }
        Err(e) => {
            return Err(SetPermissionError::DB(e));
        }
    };

    match update_member_permission(
        config.pg_pool.clone(),
        project_id,
        change_member.person_id(),
        new_permission,
    )
    .await
    {
        Ok(()) => {
            info!(
                "Updated permission for {} in {}; is now {}; request made by {}.",
                change_member.name, project.name, new_permission, requester.name
            );
            Ok((change_member, project))
        }
        Err(e) => Err(SetPermissionError::DB(e)),
    }
}

/// The errors that can occur while trying to create a project.
#[derive(Debug)]
pub(super) enum CreateProjectError {
    /// Name of the Project the requester wanted to add to
    /// (the caller does not know how that project is called yet)
    RequesterHasNoPermission,
    DB(DBError),
    Matrix(MatrixClientError),
}
impl core::fmt::Display for CreateProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RequesterHasNoPermission => {
                write!(f, "The requester does not have the necessary permissions.")
            }
            Self::DB(e) => {
                write!(f, "The DB returned this error: {e}.")
            }
            Self::Matrix(e) => {
                write!(f, "Error while communicating with matrix server: {e}")
            }
        }
    }
}
impl std::error::Error for CreateProjectError {}

pub async fn create_project(
    config: Arc<Config>,
    requester: &Person<HasID>,
    new_project_name: String,
) -> Result<Project<HasID>, CreateProjectError> {
    if requester.global_permission != UserPermission::Admin {
        return Err(CreateProjectError::RequesterHasNoPermission);
    };

    // create the new project
    let project = Project::<NoID>::new((), new_project_name);

    match add_project_prepare(config.pg_pool.clone(), project).await {
        Ok((tx, idd_project)) => {
            debug!(
                "Prepared a transaction to add project {}. Now trying to create in Matrix...",
                idd_project.name
            );
            let mut our_client = config.matrix_client.clone();
            our_client
                .ensure_room_exists(&idd_project)
                .await
                .map_err(CreateProjectError::Matrix)?;
            debug!("Successfully added project {} in Matrix. Now trying to commit the held DB transaction...", idd_project.name);
            tx.commit()
                .await
                .map_err(|e| CreateProjectError::DB(DBError::CannotCommitTransaction(e)))?;

            // both matrix and DB have agreed that all is well
            info!(
                "Created Project {}; request made by {}.",
                idd_project.name, requester.name
            );
            // then return the new project
            Ok(idd_project)
        }
        Err(e) => Err(CreateProjectError::DB(e)),
    }
}
