//! Code for communicating with matrix

use matrix_sdk::ruma::{OwnedRoomAliasId, RoomAliasId, UserId};
use matrix_sdk::{config::SyncSettings, Client};

use crate::types::HasID;
use crate::types::Person;
use crate::types::Project;

#[derive(Debug)]
pub enum MatrixClientError {
    CannotSync(matrix_sdk::Error),
    CannotGetUserIDs(matrix_sdk::IdParseError),
    CannotParseRoomAlias(matrix_sdk::IdParseError),
    CannotResolveAlias(matrix_sdk::HttpError),
    CannotCreateRoom(matrix_sdk::Error),
    RoomDoesNotExistAfterResolving(OwnedRoomAliasId),
    RoomDoesNotExist(OwnedRoomAliasId),
    CannotParseUserId(matrix_sdk::IdParseError),
    CannotAddUser(matrix_sdk::Error),
    CannotCheckMembershipStatus(matrix_sdk::Error),
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
            Self::CannotParseRoomAlias(e) => {
                write!(f, "Unable to parse room alias: {e}")
            }
            Self::CannotResolveAlias(e) => {
                write!(f, "Unable to resolve room alias: {e}")
            }
            Self::CannotCreateRoom(e) => {
                write!(f, "Unable to create room: {e}")
            }
            Self::RoomDoesNotExistAfterResolving(e) => {
                write!(
                    f,
                    "Room alias was resolved, but the room does not exist anymore: {e}"
                )
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
    servername: String,
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

    /// Ensure the room for `project` in matrix exists
    pub async fn ensure_room_exists(
        &mut self,
        project: &Project<HasID>,
    ) -> Result<(), MatrixClientError> {
        self.do_sync().await?;

        // check if the room already exists
        let local_name = project.matrix_room_alias_local();
        let room_alias = RoomAliasId::parse(format!("#{local_name}:matrix.acidresden.de"))
            .map_err(MatrixClientError::CannotParseRoomAlias)?;
        if !self
            .client
            .is_room_alias_available(&room_alias)
            .await
            .map_err(MatrixClientError::CannotResolveAlias)?
        {
            let room_id = self
                .client
                .resolve_room_alias(&room_alias)
                .await
                .map_err(MatrixClientError::CannotResolveAlias)?
                .room_id;

            if self.client.get_room(&room_id).is_none() {
                tracing::error!(
                    "Room {} was just resolved but does not exist anymore.",
                    room_alias
                );
                return Err(MatrixClientError::RoomDoesNotExistAfterResolving(
                    room_alias,
                ));
            }
        } else {
            // create a new room
            // Invite all members of this room immediately
            let mut request = matrix_sdk::ruma::api::client::room::create_room::v3::Request::new();
            request.invite = project
                .members
                .iter()
                .map(|m| format!("@{}:{}", m.0.name, self.servername).parse())
                .collect::<Result<Vec<_>, _>>()
                .map_err(MatrixClientError::CannotGetUserIDs)?;
            request.room_alias_name = Some(local_name.to_string());

            self.client
                .create_room(request)
                .await
                .map_err(MatrixClientError::CannotCreateRoom)?;
        };
        Ok(())
    }

    /// Ensure that `person` is in the room for `project` in matrix
    pub async fn ensure_user_in_room(
        &mut self,
        person: &Person<HasID>,
        project: &Project<HasID>,
    ) -> Result<(), MatrixClientError> {
        self.do_sync().await?;

        // check if the room already exists
        let local_name = project.matrix_room_alias_local();
        let room_alias = RoomAliasId::parse(format!("#{local_name}:matrix.acidresden.de"))
            .map_err(MatrixClientError::CannotParseRoomAlias)?;
        let room = if !self
            .client
            .is_room_alias_available(&room_alias)
            .await
            .map_err(MatrixClientError::CannotResolveAlias)?
        {
            let room_id = self
                .client
                .resolve_room_alias(&room_alias)
                .await
                .map_err(MatrixClientError::CannotResolveAlias)?
                .room_id;

            match self.client.get_room(&room_id) {
                None => {
                    tracing::error!(
                        "Room {} was just resolved but does not exist anymore.",
                        room_alias
                    );
                    return Err(MatrixClientError::RoomDoesNotExistAfterResolving(
                        room_alias,
                    ));
                }
                Some(x) => x,
            }
        } else {
            return Err(MatrixClientError::RoomDoesNotExist(room_alias));
        };
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
                    _ => {
                        // there is nothing more we can do - user has to accept the invite or is banned
                        return Ok(());
                    }
                }
            }
            None => {
                // User is not in room => invite
            }
        };
        match room.invite_user_by_id(&user_id).await {
            Ok(()) => {
                tracing::info!("Invited {} to Matrix-Room {}", user_id, room_alias);
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
        person: &Person<HasID>,
        project: &Project<HasID>,
    ) -> Result<(), MatrixClientError> {
        self.do_sync().await?;

        // check if the room already exists
        let local_name = project.matrix_room_alias_local();
        let room_alias = RoomAliasId::parse(format!("#{local_name}:matrix.acidresden.de"))
            .map_err(MatrixClientError::CannotParseRoomAlias)?;
        let room = if !self
            .client
            .is_room_alias_available(&room_alias)
            .await
            .map_err(MatrixClientError::CannotResolveAlias)?
        {
            let room_id = self
                .client
                .resolve_room_alias(&room_alias)
                .await
                .map_err(MatrixClientError::CannotResolveAlias)?
                .room_id;

            match self.client.get_room(&room_id) {
                None => {
                    tracing::error!(
                        "Room {} was just resolved but does not exist anymore.",
                        room_alias
                    );
                    return Err(MatrixClientError::RoomDoesNotExistAfterResolving(
                        room_alias,
                    ));
                }
                Some(x) => x,
            }
        } else {
            return Err(MatrixClientError::RoomDoesNotExist(room_alias));
        };
        // actually remove the old member
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
                tracing::info!("Kicked {} from Matrix-Room {}", user_id, room_alias);
            }
            Err(e) => {
                return Err(MatrixClientError::CannotAddUser(e));
            }
        };

        Ok(())
    }
}
