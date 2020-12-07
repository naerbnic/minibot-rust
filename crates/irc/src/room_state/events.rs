//! Events for an IRC connection.
//!
//! Glossary: Primary is the user that is logged into the connection.

#[derive(Clone)]
pub struct MembersListUpdate {
    pub members_list: super::MembersList,
}

#[derive(Clone)]
pub struct UserJoined {
    pub user: String,
}

#[derive(Clone)]
pub struct UserLeft {
    pub user: String,
}

#[derive(Clone)]
pub struct Message {
    pub from: String,
    pub message: String,
}

#[derive(Clone)]
pub enum RoomEvent {
    MembersListUpdate(MembersListUpdate),
    UserJoined(UserJoined),
    UserLeft(UserLeft),
    Message(Message),
}
