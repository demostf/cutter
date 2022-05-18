use std::collections::{BTreeMap, HashMap};
use tf_demo_parser::demo::gameevent_gen::{
    GameEvent, RocketJumpEvent, RocketJumpLandedEvent, StickyJumpEvent, StickyJumpLandedEvent,
};
use tf_demo_parser::demo::message::gameevent::GameEventMessage;
use tf_demo_parser::demo::message::packetentities::PacketEntitiesMessage;
use tf_demo_parser::demo::message::usermessage::UserMessage;
use tf_demo_parser::demo::message::Message;
use tf_demo_parser::demo::packet::stringtable::StringTableEntry;
use tf_demo_parser::demo::parser::analyser::{UserId, UserInfo};
use tf_demo_parser::demo::parser::MessageHandler;
use tf_demo_parser::demo::sendprop::{SendPropIdentifier, SendPropValue};
use tf_demo_parser::{MessageType, ParserState, ReadResult, Stream};

#[derive(Debug)]
pub struct Highlight {
    pub tick: u32,
    pub user: UserId,
    pub source: HighlightSource,
}

#[derive(Debug)]
pub enum HighlightSource {
    Prec,
    AirShot,
}

#[derive(Default)]
pub struct HighlightAnalyser {
    highlights: Vec<Highlight>,
    explosive_jumping: HashMap<UserId, bool>,
    users: BTreeMap<UserId, UserInfo>,
}

impl HighlightAnalyser {
    fn is_explosive_jumping(&self, user: UserId) -> bool {
        self.explosive_jumping
            .get(&user)
            .copied()
            .unwrap_or_default()
    }

    fn parse_user_info(&mut self, text: Option<&str>, data: Option<Stream>) -> ReadResult<()> {
        if let Some(user_info) =
            tf_demo_parser::demo::data::UserInfo::parse_from_string_table(text, data)?
        {
            self.users
                .entry(user_info.player_info.user_id.into())
                .and_modify(|info| {
                    info.entity_id = user_info.entity_id;
                })
                .or_insert_with(|| user_info.into());
        }

        Ok(())
    }
}

impl MessageHandler for HighlightAnalyser {
    type Output = Vec<Highlight>;

    fn does_handle(_message_type: MessageType) -> bool {
        true
    }

    fn handle_message(&mut self, message: &Message, tick: u32) {
        match message {
            Message::GameEvent(GameEventMessage {
                event: GameEvent::PlayerHurt(hit),
                ..
            }) if hit.attacker != hit.user_id => {
                let user = hit.user_id.into();
                if self.is_explosive_jumping(user) && hit.damage_amount > 50 {
                    dbg!(hit);
                    self.highlights.push(Highlight {
                        tick,
                        user,
                        source: HighlightSource::AirShot,
                    })
                }
            }
            Message::GameEvent(GameEventMessage {
                event: GameEvent::RocketJump(RocketJumpEvent { user_id, .. }),
                ..
            })
            | Message::GameEvent(GameEventMessage {
                event: GameEvent::StickyJump(StickyJumpEvent { user_id, .. }),
                ..
            }) => {
                let user_id = (*user_id).into();
                self.explosive_jumping.insert(user_id, true);
            }
            Message::GameEvent(GameEventMessage {
                event: GameEvent::RocketJumpLanded(RocketJumpLandedEvent { user_id, .. }),
                ..
            })
            | Message::GameEvent(GameEventMessage {
                event: GameEvent::StickyJumpLanded(StickyJumpLandedEvent { user_id, .. }),
                ..
            }) => {
                let user_id = (*user_id).into();
                self.explosive_jumping.insert(user_id, false);
            }
            Message::GameEvent(GameEventMessage {
                event: GameEvent::PlayerSpawn(spawn),
                ..
            }) => {
                let user_id = spawn.user_id.into();
                self.explosive_jumping.insert(user_id, false);
            }
            Message::UserMessage(UserMessage::SayText2(text)) => {
                if text.text == "[P-REC] Bookmark." {
                    self.highlights.push(Highlight {
                        tick,
                        user: text.client,
                        source: HighlightSource::Prec,
                    })
                }
            }
            _ => {}
        }
    }

    fn handle_string_entry(&mut self, table: &str, _index: usize, entry: &StringTableEntry) {
        if table == "userinfo" {
            let _ = self.parse_user_info(
                entry.text.as_ref().map(|s| s.as_ref()),
                entry.extra_data.as_ref().map(|data| data.data.clone()),
            );
        }
    }

    fn into_output(self, _state: &ParserState) -> Self::Output {
        dbg!(self.users);
        self.highlights
    }
}
