// Temporarily disable unused functions to be able to track real issues
#![allow(dead_code)]

use super::events::{self, MembersListUpdate, RoomEvent};
use crate::futures_util::event_sink::EventSink;
use futures::channel::mpsc;
use futures::prelude::*;
use std::collections::{btree_map, BTreeMap, BTreeSet};

pub struct UserState {
    display_name: String,
    // For future usage:
    //
    // is_mod: bool,
    // is_admin: bool,
    // is_broadcaster: bool,
    // is_global_mod: bool,
    // is_moderator: bool,
    // is_subscriber: bool,
    // is_staff: bool,
    // is_turbo: bool,
}

pub struct BigRoomMembersState {
    num_members: u32,
    recent_users: Vec<(String, UserState)>,
}

#[derive(Clone)]
pub enum MembersList {
    Lots(u32),
    Users(Vec<String>),
}

pub enum MembersState {
    /// Above a certain number, Twitch doesn't keep track of room membership anymore, nor does it give any updates.
    /// This keeps track of the number of members without tracking the actual names.
    Lots(BigRoomMembersState),
    Users(BTreeMap<String, Option<UserState>>),
}

impl MembersState {
    pub fn new() -> Self {
        MembersState::Users(BTreeMap::new())
    }

    pub fn from_list(members_list: MembersList) -> Self {
        let mut result = MembersState::new();
        result.update(members_list);
        result
    }

    pub fn to_list(&self) -> MembersList {
        todo!()
    }

    pub fn update(&mut self, members_list: MembersList) {
        match self {
            MembersState::Users(members) => match members_list {
                MembersList::Users(new_members) => {
                    let mut dropped_users: BTreeSet<String> = members.keys().cloned().collect();
                    for new_member in new_members {
                        use btree_map::Entry;
                        match members.entry(new_member) {
                            Entry::Vacant(vac) => {
                                vac.insert(None);
                            }
                            Entry::Occupied(occ) => {
                                dropped_users.remove(occ.key());
                            }
                        }
                    }

                    for dropped_user in dropped_users {
                        members.remove(&dropped_user);
                    }
                }
                MembersList::Lots(num_members) => {
                    *self = MembersState::Lots(BigRoomMembersState {
                        num_members,
                        recent_users: Vec::new(),
                    })
                }
            },

            MembersState::Lots(state) => match members_list {
                MembersList::Lots(num_members) => state.num_members = num_members,
                MembersList::Users(new_members) => {
                    *self =
                        MembersState::Users(new_members.into_iter().map(|n| (n, None)).collect())
                }
            },
        }
    }
}

pub struct RoomState {
    members: Option<MembersState>,
    events_sink: mpsc::Sender<super::events::RoomEvent>,
    events_channel: EventSink<super::events::RoomEvent>,
}

impl RoomState {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel(3);
        RoomState {
            members: None,
            events_sink: tx,
            events_channel: EventSink::new(rx),
        }
    }

    pub async fn update_user_state(&mut self, _user: &str, _display_name: &str) {}

    pub async fn notify_members_list(&mut self, members_list: MembersList) {
        self.events_sink
            .send(RoomEvent::MembersListUpdate(MembersListUpdate {
                members_list: members_list.clone(),
            }))
            .await
            .unwrap();

        match &mut self.members {
            Some(members) => members.update(members_list),
            None => self.members = Some(MembersState::from_list(members_list)),
        }
    }

    pub async fn notify_join_room(&mut self, user: &str) {
        self.events_sink
            .send(RoomEvent::UserJoined(events::UserJoined {
                user: user.to_string(),
            }))
            .await
            .unwrap();
    }

    pub async fn notify_part_room(&mut self, user: &str) {
        self.events_sink
            .send(RoomEvent::UserLeft(events::UserLeft {
                user: user.to_string(),
            }))
            .await
            .unwrap();
    }

    pub async fn notify_message(&mut self, user: &str, message: &str) {
        self.events_sink
            .send(RoomEvent::Message(events::Message {
                from: user.to_string(),
                message: message.to_string(),
            }))
            .await
            .unwrap();
    }

    pub async fn add_listener(&mut self, mut listener: mpsc::Sender<RoomEvent>) {
        // Get the listener up to speed by sending an update event for the
        // current state of the room (if there is any).
        if let Some(members_state) = &self.members {
            let send_result = listener
                .send(RoomEvent::MembersListUpdate(MembersListUpdate {
                    members_list: members_state.to_list(),
                }))
                .await;

            // An error indicates the sender was disconnected. No point in
            // adding it.
            if send_result.is_err() {
                return;
            }
        }

        self.events_channel.add_sink(listener);
    }
}

pub struct ConnectionState {
    user: String,
    rooms: BTreeMap<String, RoomState>,
}

impl ConnectionState {
    pub fn notify_join_room(&mut self, room: String) -> &mut RoomState {
        use btree_map::Entry;
        match self.rooms.entry(room) {
            Entry::Occupied(occ) => occ.into_mut(),
            Entry::Vacant(vac) => vac.insert(RoomState::new()),
        }
    }

    pub fn get_room_mut(&mut self, room: &str) -> Option<&mut RoomState> {
        self.rooms.get_mut(room)
    }

    pub fn get_room(&self, room: &str) -> Option<&RoomState> {
        self.rooms.get(room)
    }

    pub fn notify_whisper(&mut self, _user: &str, _message: &str) {
        todo!()
    }
}
