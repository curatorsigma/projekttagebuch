//! Code for communicating with matrix

use matrix_sdk::ruma::{OwnedRoomId, RoomId, UserId};
use matrix_sdk::{config::SyncSettings, Client};
use tracing::warn;

use crate::types::{DbNoMatrix, FullId, MatrixNoDb, NoId, Person};
use crate::types::Project;

#[derive(Debug)]
pub enum MatrixClientError {
    CannotSync(matrix_sdk::Error),
    CannotGetUserIDs(matrix_sdk::IdParseError),
    CannotParseRoomId(matrix_sdk::IdParseError),
    CannotCreateRoom(matrix_sdk::Error),
    RoomDoesNotExist(OwnedRoomId),
    CannotParseUserId(matrix_sdk::IdParseError),
    CannotAddUser(matrix_sdk::Error),
    CannotCheckMembershipStatus(matrix_sdk::Error),
    UserIsBanned,
    StateUnknown,
}
impl core::fmt::Display for MatrixClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CannotSync(e) => {
                write!(f, "Unable to sync from Matrix Server: {e}")
            }
            Self::CannotGetUserIDs(e) => {
                write!(f, "Unable to get user IDs from their names: {e}")
            }
            Self::CannotParseRoomId(e) => {
                write!(f, "Unable to parse room id: {e}")
            }
            Self::CannotCreateRoom(e) => {
                write!(f, "Unable to create room: {e}")
            }
            Self::RoomDoesNotExist(e) => {
                write!(f, "Room with this alias does not exist: {e}")
            }
            Self::CannotParseUserId(e) => {
                write!(f, "Unable to parse user-id: {e}")
            }
            Self::CannotAddUser(e) => {
                write!(f, "Unable to add a user to a room: {e}")
            }
            Self::CannotCheckMembershipStatus(e) => {
                write!(f, "Unable to check membership status for a user: {e}")
            }
            Self::UserIsBanned => {
                write!(f, "The user is banned from a room we want to invite them into.")
            }
            Self::StateUnknown => {
                write!(f, "The MembershipState of a user is of a type that is not documented.")
            }
        }
    }
}
impl std::error::Error for MatrixClientError {}

/// The Matrix client used in this application to make requests to Matrix.
///
/// Note: [`Client`] is just a wrapper for Arc, so it is fine to clone this [`MatrixClient`]
/// whenever we need this - we thus avoid Mutexing the entire config or something similar.
#[derive(Debug, Clone)]
pub(crate) struct MatrixClient {
    client: Client,
    last_sync_token: Option<String>,
    /// The servername used in our matrix server
    servername: String,
    /// The servername for our element server (used to generate urls pointing to rooms)
    element_servername: String,
}
impl MatrixClient {
    pub fn new(client: Client, servername: String, element_servername: String) -> Self {
        Self {
            client,
            last_sync_token: None,
            servername,
            element_servername,
        }
    }

    pub fn matrix_server(&self) -> &str {
        &self.servername
    }

    pub fn element_server(&self) -> &str {
        &self.element_servername
    }

    /// Sync once and set the new sync checkpoint if successfull
    async fn do_sync(&mut self) -> Result<(), MatrixClientError> {
        let settings = if let Some(token) = &self.last_sync_token {
            SyncSettings::new().token(token)
        } else {
            SyncSettings::default()
        };
        let response = self
            .client
            .sync_once(settings)
            .await
            .map_err(MatrixClientError::CannotSync)?;
        self.last_sync_token = Some(response.next_batch);

        Ok(())
    }

    /// Create a new room with the name given by `project`
    pub async fn create_room(
        &mut self,
        project: Project<NoId>,
    ) -> Result<Project<MatrixNoDb>, MatrixClientError> {
        self.do_sync().await?;

        // create a new room
        // Invite all members of this room immediately
        let mut request = matrix_sdk::ruma::api::client::room::create_room::v3::Request::new();
        request.invite = project
            .members
            .iter()
            .map(|m| format!("@{}:{}", m.0.name, self.servername).parse())
            .collect::<Result<Vec<_>, _>>()
            .map_err(MatrixClientError::CannotGetUserIDs)?;
        request.name = Some(project.name.clone());

        let room = self.client
            .create_room(request)
            .await
            .map_err(MatrixClientError::CannotCreateRoom)?;

        Ok(project.set_matrix_id(room.room_id().as_str().to_owned()))
    }

    /// Ensure that `person` is in the room for `project` in matrix
    pub async fn ensure_user_in_room(
        &mut self,
        person: &Person<DbNoMatrix>,
        project: &Project<FullId>,
    ) -> Result<(), MatrixClientError> {
        self.do_sync().await?;

        // check if the room already exists
        let room_id = RoomId::parse(project.matrix_id()).map_err(MatrixClientError::CannotParseRoomId)?;
        let room = self.client.get_room(&room_id).ok_or_else(|| { MatrixClientError::RoomDoesNotExist(room_id.clone()) })?;

        // actually invite the new member
        let user_id = UserId::parse(format!("@{}:{}", person.name, self.servername))
            .map_err(MatrixClientError::CannotParseUserId)?;
        // check that we only invite users that are not already joined or invited
        let already_in_room = room
            .get_member(&user_id)
            .await
            .map_err(MatrixClientError::CannotCheckMembershipStatus)?;
        match already_in_room {
            Some(ref member_obj) => {
                match member_obj.membership() {
                    matrix_sdk::ruma::events::room::member::MembershipState::Join => {
                        // already done
                        return Ok(());
                    }
                    matrix_sdk::ruma::events::room::member::MembershipState::Knock => {
                        // User has already knocked => invite
                    }
                    matrix_sdk::ruma::events::room::member::MembershipState::Leave => {
                        // the user was kicked or has left before => reinvite
                    }
                    matrix_sdk::ruma::events::room::member::MembershipState::Ban => {
                        warn!("Trying to invite user {} to room {} ({}), but they are banned from that room.", user_id, project.name, room.room_id());
                        return Err(MatrixClientError::UserIsBanned);
                    }
                    matrix_sdk::ruma::events::room::member::MembershipState::Invite => {
                        warn!("Trying to invite user {} to room {} ({}), but they are already invited.", user_id, project.name, room.room_id());
                        return Ok(());
                    }
                    // evil evil matrix_sdk has marked this enum as non-exhaustive
                    _ => {
                        warn!("Hit an unknown type for matrix_sdk::ruma::events::room::member::MembershipState.");
                        return Err(MatrixClientError::StateUnknown);
                    }
                }
            }
            None => {
                // User is not in room => invite
            }
        };
        match room.invite_user_by_id(&user_id).await {
            Ok(()) => {
                tracing::info!("Invited {} to room {} ({}).", user_id, project.name, room.room_id());
            }
            Err(e) => {
                return Err(MatrixClientError::CannotAddUser(e));
            }
        };

        Ok(())
    }

    /// Ensure that `person` is not in the room for `project` in matrix
    pub async fn ensure_user_not_in_room(
        &mut self,
        person: &Person<DbNoMatrix>,
        project: &Project<FullId>,
    ) -> Result<(), MatrixClientError> {
        self.do_sync().await?;

        let room_id = RoomId::parse(project.matrix_id()).map_err(MatrixClientError::CannotParseRoomId)?;
        let room = self.client.get_room(&room_id).ok_or_else(|| { MatrixClientError::RoomDoesNotExist(room_id.clone()) })?;
        // remove the old member
        let user_id = UserId::parse(format!("@{}:{}", person.name, self.servername))
            .map_err(MatrixClientError::CannotParseUserId)?;
        // check that we only remove users that are actually in the room
        let already_in_room = room
            .get_member(&user_id)
            .await
            .map_err(MatrixClientError::CannotCheckMembershipStatus)?;
        match already_in_room {
            Some(ref member_obj) => {
                match member_obj.membership() {
                    matrix_sdk::ruma::events::room::member::MembershipState::Join => {
                        // User is Joined => remove them
                    }
                    matrix_sdk::ruma::events::room::member::MembershipState::Invite => {
                        // User has been invited => retract the invite (same as kicking)
                    }
                    _ => {
                        // user is not yet in the room and cannot be removed (banned, left, knocked
                        // but not invited)
                        return Ok(());
                    }
                }
            }
            None => {
                // User is not in room
                return Ok(());
            }
        };
        match room
            .kick_user(&user_id, Some("projekttagebuch Automatisierung"))
            .await
        {
            Ok(()) => {
                tracing::info!("Kicked {} from Matrix-Room {} ({})", user_id, project.name, room_id);
            }
            Err(e) => {
                return Err(MatrixClientError::CannotAddUser(e));
            }
        };

        Ok(())
    }
}
