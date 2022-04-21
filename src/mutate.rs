use std::mem::take;
use tf_demo_parser::demo::message::packetentities::PacketEntity;
use tf_demo_parser::demo::message::Message;
use tf_demo_parser::demo::packet::Packet;

pub trait PacketMutator {
    fn mutate_packet(&self, packet: &mut Packet);
}

pub trait MessageMutator {
    fn mutate_message(&self, message: &mut Message);
}

pub trait MessageFilter {
    fn filter(&self, message: &Message) -> bool;
}

struct PacketMessageMutator<T: MessageMutator> {
    mutator: T,
}

impl<T: MessageMutator> PacketMutator for PacketMessageMutator<T> {
    fn mutate_packet(&self, packet: &mut Packet) {
        if let Packet::Message(msg_packet) = packet {
            msg_packet
                .messages
                .iter_mut()
                .for_each(|msg| self.mutator.mutate_message(msg));
        }
    }
}

impl<F: Fn(&mut Packet)> PacketMutator for F {
    fn mutate_packet(&self, packet: &mut Packet) {
        self(packet)
    }
}

impl<T: MessageMutator> From<T> for PacketMessageMutator<T> {
    fn from(mutator: T) -> Self {
        PacketMessageMutator { mutator }
    }
}

impl<F: Fn(&mut Message)> MessageMutator for F {
    fn mutate_message(&self, message: &mut Message) {
        self(message)
    }
}

struct PacketMessageFilter<T: MessageFilter> {
    filter: T,
}

impl<T: MessageFilter> PacketMutator for PacketMessageFilter<T> {
    fn mutate_packet(&self, packet: &mut Packet) {
        if let Packet::Message(msg_packet) = packet {
            let messages = take(&mut msg_packet.messages);
            msg_packet.messages = messages
                .into_iter()
                .filter(|msg| self.filter.filter(msg))
                .collect();
        }
    }
}

impl<T: MessageFilter> From<T> for PacketMessageFilter<T> {
    fn from(filter: T) -> Self {
        PacketMessageFilter { filter }
    }
}

impl<F: Fn(&Message) -> bool> MessageFilter for F {
    fn filter(&self, message: &Message) -> bool {
        self(message)
    }
}

#[derive(Default)]
pub struct MutatorList {
    mutators: Vec<Box<dyn PacketMutator>>,
}

impl MutatorList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_packet_mutator<M: PacketMutator + 'static>(&mut self, mutator: M) {
        self.mutators.push(Box::new(mutator))
    }

    pub fn push_message_mutator<M: PacketMutator + 'static, T: Into<M>>(&mut self, mutator: T) {
        self.mutators.push(Box::new(mutator.into()))
    }

    pub fn push_message_filter<M: MessageFilter + 'static>(&mut self, filter: M) {
        self.mutators
            .push(Box::new(PacketMessageFilter::from(filter)))
    }
}

impl PacketMutator for MutatorList {
    fn mutate_packet(&self, packet: &mut Packet) {
        for mutator in self.mutators.iter() {
            mutator.mutate_packet(packet);
        }
    }
}
