#![allow(unused_imports)]

mod entity;
mod mutate;
mod string_tables;
mod utils;

use crate::entity::ActiveEntities;
use crate::mutate::{MessageMutator, MutatorList, PacketMutator};
use crate::string_tables::StringTablesUpdates;
use crate::utils::set_panic_hook;
use bitbuffer::{BitRead, BitWrite, BitWriteStream, LittleEndian};
use std::cmp::{max, min};
use std::collections::BTreeSet;
use std::convert::TryInto;
use std::iter::once;
use std::mem::take;
use tf_demo_parser::demo::header::Header;
use tf_demo_parser::demo::message::packetentities::UpdateType::Delete;
use tf_demo_parser::demo::message::packetentities::{EntityId, PacketEntitiesMessage, UpdateType};
use tf_demo_parser::demo::message::usermessage::UserMessageType;
use tf_demo_parser::demo::message::{Message, NetTickMessage};
use tf_demo_parser::demo::packet::message::{MessagePacket, MessagePacketMeta};
use tf_demo_parser::demo::packet::stop::StopPacket;
use tf_demo_parser::demo::packet::PacketType::StringTables;
use tf_demo_parser::demo::packet::{Packet, PacketType};
use tf_demo_parser::demo::parser::{DemoHandler, Encode, NullHandler, RawPacketStream};
use tf_demo_parser::{Demo, MessageType};
use wasm_bindgen::prelude::*;
use web_sys::console;

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

const PRESERVE_PACKETS: &[PacketType] = &[
    PacketType::Signon,
    PacketType::DataTables,
    PacketType::StringTables,
    PacketType::SyncTick,
];

#[wasm_bindgen]
pub fn cut(input: &[u8], start_tick: u32, end_tick: u32) -> Vec<u8> {
    set_panic_hook();
    let mut out_buffer = Vec::with_capacity(input.len());
    {
        let mut out_stream = BitWriteStream::new(&mut out_buffer, LittleEndian);

        let demo = Demo::new(&input);
        let mut stream = demo.get_stream();
        let mut header = Header::read(&mut stream).unwrap();

        let start_tick = min(header.ticks - 10, start_tick);
        let end_tick = min(header.ticks, end_tick);
        let duration_per_tick = header.ticks as f32 / header.duration;

        header.ticks = end_tick - start_tick;
        header.duration = (end_tick - start_tick) as f32 * duration_per_tick;
        header.write(&mut out_stream).unwrap();

        let mut packets = RawPacketStream::new(stream.clone());
        let mut start_handler = DemoHandler::default();
        start_handler.handle_header(&header);

        let mut handler = DemoHandler::default();
        handler.handle_header(&header);

        let start_state = skip_start(&mut start_handler, &mut packets, start_tick);

        for packet in start_state.start_packets {
            packet
                .encode(&mut out_stream, &handler.state_handler)
                .unwrap();
            handler.handle_packet(packet).unwrap();
        }

        let mut next = packets.next(&start_handler.state_handler).unwrap().unwrap();
        let mut delta_tick = 0;
        let mut max = 0;
        let mut baseline = 0;
        if let Packet::Message(MessagePacket { messages, .. }) = &next {
            for msg in messages {
                if let Message::PacketEntities(PacketEntitiesMessage {
                    delta: Some(delta),
                    max_entries,
                    base_line,
                    ..
                }) = msg
                {
                    max = *max_entries;
                    baseline = *base_line;
                    delta_tick = delta.get();
                }
            }
        } else {
            panic!("first packet is not a MessagePacket, pick a different start tick")
        }

        let start_entities = start_state.entities.entity_ids();

        let string_table_updates = start_state
            .table_updates
            .encode()
            .into_iter()
            .map(|msg| Message::UpdateStringTable(msg));
        let (baseline_updates, entity_update, removed_update) = start_state
            .entities
            .encode(&start_handler.state_handler, delta_tick - 2);
        let baseline_updates = baseline_updates.into_iter().map(Message::PacketEntities);
        let start_packets = string_table_updates
            .chain(baseline_updates)
            .map(|msg| msg_packet(vec![net_tick(delta_tick - 2), msg]))
            .chain(once(msg_packet(vec![
                net_tick(delta_tick - 1),
                Message::PacketEntities(entity_update),
            ])))
            .chain(once(Packet::Message(MessagePacket {
                messages: vec![
                    net_tick(delta_tick),
                    Message::PacketEntities(removed_update),
                ],
                ..MessagePacket::default()
            })));
        for packet in start_packets {
            packet
                .encode(&mut out_stream, &handler.state_handler)
                .unwrap();
            handler.handle_packet(packet).unwrap();
        }

        // create the net ticks needed for later deltas
        let fill_ticks = ((delta_tick + 1)..=start_state.server_tick)
            .into_iter()
            .map(|tick| net_tick(tick));
        let fill_packets = fill_ticks.map(|msg| {
            Packet::Message(MessagePacket {
                messages: vec![
                    msg,
                    Message::PacketEntities(PacketEntitiesMessage {
                        max_entries: max,
                        delta: Some((delta_tick - 1).try_into().unwrap()),
                        base_line: baseline,
                        ..PacketEntitiesMessage::default()
                    }),
                ],
                ..MessagePacket::default()
            })
        });
        for packet in fill_packets {
            packet
                .encode(&mut out_stream, &handler.state_handler)
                .unwrap();
        }

        let mut mutators = MutatorList::new();
        mutators.push_message_filter(|message: &Message| {
            if let Message::UserMessage(usr_message) = message {
                UserMessageType::CloseCaption != usr_message.message_type()
            } else {
                true
            }
        });
        mutators.push_message_mutator(DeleteFilter::new(start_entities, start_state.server_tick));
        mutators.push_packet_mutator(move |packet: &mut Packet| {
            packet.set_tick(packet.tick() - start_tick)
        });

        mutators.mutate_packet(&mut next);
        next.encode(&mut out_stream, &handler.state_handler)
            .unwrap();
        handler.handle_packet(next).unwrap();

        while let Some(mut packet) = packets.next(&handler.state_handler).unwrap() {
            let original_tick = packet.tick();

            mutators.mutate_packet(&mut packet);

            if packet.packet_type() != PacketType::ConsoleCmd {
                packet
                    .encode(&mut out_stream, &handler.state_handler)
                    .unwrap();
            }
            handler.handle_packet(packet).unwrap();

            if original_tick >= end_tick {
                break;
            }
        }
        PacketType::Stop.write(&mut out_stream).unwrap();
        StopPacket {
            tick: end_tick - start_tick,
        }
        .encode(&mut out_stream, &handler.state_handler)
        .unwrap();
    }
    out_buffer
}

struct StartState<'a> {
    entities: ActiveEntities,
    table_updates: StringTablesUpdates,
    start_packets: Vec<Packet<'a>>,
    server_tick: u32,
}

fn skip_start<'a>(
    handler: &mut DemoHandler<'a, NullHandler>,
    packets: &mut RawPacketStream<'a>,
    start_tick: u32,
) -> StartState<'a> {
    let mut entities = ActiveEntities::default();
    let mut table_updates = StringTablesUpdates::default();
    let mut start_packets = Vec::with_capacity(6);
    let mut server_tick = 0;

    while let Some(packet) = packets.next(&handler.state_handler).unwrap() {
        if PRESERVE_PACKETS.contains(&packet.packet_type()) {
            start_packets.push(packet.clone());
            handler.handle_packet(packet).unwrap();
        } else {
            if let Packet::Message(message_packet) = &packet {
                for msg in &message_packet.messages {
                    table_updates.handle_message(&msg);
                    match msg {
                        Message::PacketEntities(msg) => {
                            entities.handle_message(msg, &handler.state_handler);
                        }
                        Message::NetTick(NetTickMessage { tick, .. }) => {
                            server_tick = *tick;
                        }
                        _ => {}
                    }
                }
            }
            let tick = packet.tick();
            handler.handle_packet(packet).unwrap();

            if tick >= start_tick {
                break;
            }
        }
    }

    StartState {
        entities,
        table_updates,
        start_packets,
        server_tick,
    }
}

struct DeleteFilter {
    current_entities: BTreeSet<EntityId>,
    till_delta: u32,
}

impl DeleteFilter {
    pub fn new(current_entities: BTreeSet<EntityId>, till_delta: u32) -> Self {
        DeleteFilter {
            current_entities,
            till_delta,
        }
    }
}

impl MessageMutator for DeleteFilter {
    fn mutate_message(&self, message: &mut Message) {
        if let Message::PacketEntities(message) = message {
            if let Some(delta) = message.delta {
                if delta.get() < self.till_delta {
                    let packet_entities = take(&mut message.entities);
                    message.entities = packet_entities
                        .into_iter()
                        .filter(|ent| match ent.update_type {
                            UpdateType::Delete | UpdateType::Leave => {
                                self.current_entities.contains(&ent.entity_index)
                            }
                            _ => true,
                        })
                        .collect();
                }
            }
        }
    }
}

fn msg_packet(messages: Vec<Message>) -> Packet {
    Packet::Message(MessagePacket {
        messages,
        ..MessagePacket::default()
    })
}

fn net_tick(tick: u32) -> Message<'static> {
    Message::NetTick(NetTickMessage {
        tick,
        frame_time: 1881,
        std_dev: 263,
    })
}
