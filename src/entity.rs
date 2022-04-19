use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::iter::once;
use std::mem::replace;
use tf_demo_parser::demo::message::packetentities::{
    EntityId, PacketEntitiesMessage, PacketEntity, UpdateType,
};
use tf_demo_parser::demo::sendprop::SendPropIdentifier;
use tf_demo_parser::ParserState;

#[derive(Default)]
pub struct ActiveEntities {
    entities: BTreeMap<EntityId, PacketEntity>,
    max_entities: u16,
}

impl ActiveEntities {
    pub fn handle_message(&mut self, msg: &PacketEntitiesMessage, state: &ParserState) {
        self.max_entities = self.max_entities.max(msg.max_entries);
        for entity in &msg.entities {
            if entity.update_type == UpdateType::Delete || entity.update_type == UpdateType::Leave {
                self.remove_entity(entity.entity_index);
            } else {
                self.handle_entity(entity, state);
            }
        }
        for deleted in msg.removed_entities.iter() {
            self.remove_entity(*deleted);
        }
    }

    fn remove_entity(&mut self, entity_index: EntityId) {
        self.entities.remove(&entity_index);
    }

    fn handle_entity(&mut self, entity: &PacketEntity, state: &ParserState) {
        self.entities
            .entry(entity.entity_index)
            .and_modify(|existing| {
                if existing.serial_number != entity.serial_number
                    && existing.server_class != entity.server_class
                {
                    // todo: do baselines need to be cleanup up or updated here?
                    *existing = entity.clone();
                    existing.update_type = UpdateType::Enter;
                } else {
                    debug_assert_eq!(
                        state.server_classes[usize::from(existing.server_class)],
                        state.server_classes[usize::from(entity.server_class)]
                    );
                    if existing.serial_number != entity.serial_number {
                        existing.serial_number = entity.serial_number;
                        existing.update_type = UpdateType::Enter;
                    }
                    existing.apply_update(&entity.props);
                }
            })
            .or_insert_with(|| entity.clone());
    }

    pub fn entity_ids(&self) -> BTreeSet<EntityId> {
        self.entities.keys().copied().collect()
    }

    pub fn encode(
        mut self,
        state: &ParserState,
    ) -> impl IntoIterator<Item = PacketEntitiesMessage> {
        // baselines in reverse order
        let mut baselines = [
            encode_entities(
                state.instance_baselines[1]
                    .clone()
                    .into_values()
                    .collect::<Vec<_>>(),
                self.max_entities,
            ),
            encode_entities(
                state.instance_baselines[0]
                    .clone()
                    .into_values()
                    .collect::<Vec<_>>(),
                self.max_entities,
            ),
        ];
        for entity in self.entities.values_mut() {
            entity.update_type = UpdateType::Enter;
        }
        let entities = encode_entities(
            self.entities.into_values().collect::<Vec<_>>(),
            self.max_entities,
        );

        baselines[0].updated_base_line = true;
        baselines[1].updated_base_line = true;
        baselines[1].base_line = 1;

        baselines.into_iter().chain(once(entities))
    }
}

fn encode_entities(mut entities: Vec<PacketEntity>, max_entries: u16) -> PacketEntitiesMessage {
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
