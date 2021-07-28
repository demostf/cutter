use std::collections::HashMap;
use std::mem::replace;
use tf_demo_parser::demo::message::packetentities::{
    EntityId, PacketEntitiesMessage, PacketEntity, PVS,
};
use tf_demo_parser::ParserState;

#[derive(Default)]
pub struct ActiveEntities {
    entities: HashMap<EntityId, PacketEntity>,
}

impl ActiveEntities {
    pub fn handle_message(&mut self, msg: &PacketEntitiesMessage, state: &ParserState) {
        for entity in &msg.entities {
            if entity.pvs == PVS::Delete || entity.pvs == PVS::Leave {
                self.entities.remove(&entity.entity_index);
            } else {
                self.handle_entity(entity, state);
            }
        }
        for deleted in msg.removed_entities.iter() {
            self.entities.remove(deleted);
        }
    }

    fn handle_entity(&mut self, entity: &PacketEntity, state: &ParserState) {
        if entity.pvs == PVS::Enter {
            self.entities.insert(entity.entity_index, entity.clone());
        } else {
            self.entities
                .entry(entity.entity_index)
                .and_modify(|existing| update_entity(existing, entity, state))
                .or_insert_with(|| entity.clone());
        }
    }

    pub fn encode(self) -> PacketEntitiesMessage {
        let max_entries = self.entities.len() as u16;

        let mut entities = self
            .entities
            .into_iter()
            .map(|(_k, v)| v)
            .collect::<Vec<_>>();
        entities.sort_by(|a, b| a.entity_index.cmp(&b.entity_index));

        PacketEntitiesMessage {
            entities,
            removed_entities: vec![],
            max_entries,
            delta: None,
            base_line: 0,
            updated_base_line: false,
        }
    }
}

fn update_entity(old: &mut PacketEntity, new: &PacketEntity, _state: &ParserState) {
    for prop in &new.props {
        match old
            .props
            .iter_mut()
            .find(|existing| existing.index == prop.index)
        {
            Some(existing) => existing.value = prop.value.clone(),
            None => old.props.push(prop.clone()),
        }
    }
}
