//! Events for an IRC connection.
//!
//! Glossary: Primary is the user that is logged into the connection.
pub struct MembersListUpdate {
    members_list: super::MembersList,
}

pub struct UserJoined {
    user: String,
}

pub struct UserLeft {
    user: String,
}

pub struct Message {
    from: String,
    message: String,
}

pub enum RoomEvent {
    MembersListUpdate(MembersListUpdate),
    UserJoined(UserJoined),
    UserLeft(UserLeft),
    Message(Message),
}
