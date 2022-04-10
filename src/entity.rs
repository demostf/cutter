use std::collections::HashMap;
use std::iter::once;
use std::mem::replace;
use tf_demo_parser::demo::message::packetentities::{
    EntityId, PacketEntitiesMessage, PacketEntity, PVS,
};
use tf_demo_parser::ParserState;

#[derive(Default)]
pub struct ActiveEntities {
    baselines: [HashMap<EntityId, PacketEntity>; 2],
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
        if msg.updated_base_line {
            let old_index = msg.base_line as usize;
            let new_index = 1 - old_index;
            self.baselines.swap(0, 1);

            for entity in &msg.entities {
                if entity.pvs == PVS::Enter {
                    self.baselines[new_index].insert(entity.entity_index, entity.clone());
                }
            }
        }
    }

    fn handle_entity(&mut self, entity: &PacketEntity, state: &ParserState) {
        self.entities
            .entry(entity.entity_index)
            .and_modify(|existing| update_entity(existing, entity, state))
            .or_insert_with(|| entity.clone());
    }

    pub fn encode(self) -> impl IntoIterator<Item = PacketEntitiesMessage> {
        let mut baselines = [
            encode_entities(self.baselines[0].clone().into_values().collect::<Vec<_>>()),
            encode_entities(self.baselines[1].clone().into_values().collect::<Vec<_>>()),
        ];
        let entities = encode_entities(self.entities.into_values().collect::<Vec<_>>());

        baselines[0].updated_base_line = true;
        baselines[0].base_line = 1; //the baseline that is updated is the other one

        baselines[1].updated_base_line = true;

        baselines.into_iter().chain(once(entities))
    }
}

fn encode_entities(mut entities: Vec<PacketEntity>) -> PacketEntitiesMessage {
    entities.sort_by(|a, b| a.entity_index.cmp(&b.entity_index));
    let max_entries = entities.len() as u16;
    PacketEntitiesMessage {
        entities,
        removed_entities: vec![],
        max_entries,
        delta: None,
        base_line: 0,
        updated_base_line: false,
    }
}

fn update_entity(old: &mut PacketEntity, new: &PacketEntity, _state: &ParserState) {
    if old.serial_number != new.serial_number {
        *old = new.clone();
        old.pvs = PVS::Enter;
    } else {
        assert_eq!(
            _state.server_classes[usize::from(old.server_class)],
            _state.server_classes[usize::from(new.server_class)]
        );
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
}
