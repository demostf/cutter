#![allow(unused_imports)]

mod entity;
mod utils;

use crate::entity::ActiveEntities;
use crate::utils::set_panic_hook;
use bitbuffer::{BitRead, BitWrite, BitWriteStream, LittleEndian};
use std::cmp::{max, min};
use tf_demo_parser::demo::header::Header;
use tf_demo_parser::demo::message::{Message, NetTickMessage};
use tf_demo_parser::demo::packet::message::{MessagePacket, MessagePacketMeta};
use tf_demo_parser::demo::packet::stop::StopPacket;
use tf_demo_parser::demo::packet::{Packet, PacketType};
use tf_demo_parser::demo::parser::{DemoHandler, Encode, NullHandler, RawPacketStream};
use tf_demo_parser::Demo;
use wasm_bindgen::prelude::*;
use web_sys::console;

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

const PRESERVE_PACKETS: &[PacketType] = &[
    PacketType::Sigon,
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

        let (entities, start_packets, last_server_tick) =
            skip_start(&mut start_handler, &mut packets, start_tick);

        for packet in start_packets {
            packet
                .encode(&mut out_stream, &handler.state_handler)
                .unwrap();
            handler.handle_packet(packet).unwrap();
        }

        let msg = entities.encode();
        let packet = Packet::Message(MessagePacket {
            tick: 0,
            messages: vec![
                Message::NetTick(NetTickMessage {
                    tick: last_server_tick,
                    frame_time: 1881,
                    std_dev: 263,
                }),
                Message::PacketEntities(msg),
            ],
            meta: MessagePacketMeta {
                flags: 0,
                view_angles: Default::default(),
                sequence_in: 0,
                sequence_out: 0,
            },
        });
        packet
            .encode(&mut out_stream, &handler.state_handler)
            .unwrap();
        handler.handle_packet(packet).unwrap();

        while let Some(mut packet) = packets.next(&handler.state_handler).unwrap() {
            let ty = packet.packet_type();
            let original_tick = packet.tick();
            packet.set_tick(max(original_tick, start_tick) - start_tick);
            if ty != PacketType::ConsoleCmd {
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
        StopPacket { tick: end_tick }
            .encode(&mut out_stream, &handler.state_handler)
            .unwrap();
    }
    out_buffer
}

fn skip_start<'a>(
    handler: &mut DemoHandler<'a, NullHandler>,
    packets: &mut RawPacketStream<'a>,
    start_tick: u32,
) -> (ActiveEntities, Vec<Packet<'a>>, u32) {
    let mut entities = ActiveEntities::default();
    let mut start_packets = Vec::with_capacity(6);
    let mut server_tick = 0;

    while let Some(packet) = packets.next(&handler.state_handler).unwrap() {
        if PRESERVE_PACKETS.contains(&packet.packet_type()) {
            start_packets.push(packet.clone());
            handler.handle_packet(packet).unwrap();
        } else {
            if let Packet::Message(message_packet) = &packet {
                for msg in &message_packet.messages {
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

    (entities, start_packets, server_tick)
}
