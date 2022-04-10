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
                self.remove_entity(entity.entity_index);
            } else {
                self.handle_entity(entity, state);
            }
        }
        for deleted in msg.removed_entities.iter() {
            self.remove_entity(*deleted);
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

    fn remove_entity(&mut self, entity_index: EntityId) {
        self.entities.remove(&entity_index);
        self.baselines[0].remove(&entity_index);
        self.baselines[1].remove(&entity_index);
    }

    fn handle_entity(&mut self, entity: &PacketEntity, state: &ParserState) {
        self.entities
            .entry(entity.entity_index)
            .and_modify(|existing| {
                if existing.serial_number != entity.serial_number {
                    *existing = entity.clone();
                    existing.pvs = PVS::Enter;
                } else {
                    assert_eq!(
                        state.server_classes[usize::from(existing.server_class)],
                        state.server_classes[usize::from(entity.server_class)]
                    );
                    for prop in &entity.props {
                        match existing
                            .props
                            .iter_mut()
                            .find(|existing| existing.index == prop.index)
                        {
                            Some(existing) => existing.value = prop.value.clone(),
                            None => existing.props.push(prop.clone()),
                        }
                    }
                }
            })
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
