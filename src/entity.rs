use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::iter::once;
use std::mem::{replace, take};
use std::num::NonZeroU32;
use tf_demo_parser::demo::message::packetentities::{
    EntityId, PacketEntitiesMessage, PacketEntity, UpdateType,
};
use tf_demo_parser::demo::packet::datatable::ClassId;
use tf_demo_parser::demo::sendprop::SendPropIdentifier;
use tf_demo_parser::ParserState;

#[derive(Default)]
pub struct ActiveEntities {
    entities: BTreeMap<EntityId, PacketEntity>,
    max_entities: u16,
    deleted_entities: BTreeSet<EntityId>,
    removed_entities: BTreeSet<EntityId>,
}

impl ActiveEntities {
    pub fn handle_message(&mut self, msg: &PacketEntitiesMessage, state: &ParserState) {
        self.max_entities = self.max_entities.max(msg.max_entries);
        for entity in &msg.entities {
            self.removed_entities.remove(&entity.entity_index);

            if entity.update_type == UpdateType::Delete {
                self.deleted_entities.insert(entity.entity_index);
            } else {
                self.deleted_entities.remove(&entity.entity_index);
            }

            if entity.update_type == UpdateType::Delete || entity.update_type == UpdateType::Leave {
                self.remove_entity(entity.entity_index);
            } else {
                self.handle_entity(entity, state);
            }
        }
        for deleted in msg.removed_entities.iter() {
            self.remove_entity(*deleted);
            self.removed_entities.insert(*deleted);
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
                    *existing = entity.clone();
                    existing.apply_update(&entity.props);
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

    pub fn baseline_ids(&self, state: &ParserState) -> BTreeSet<EntityId> {
        state.instance_baselines[0]
            .keys()
            .chain(state.instance_baselines[1].keys())
            .collect()
    }

    pub fn encode(
        mut self,
        state: &ParserState,
        delta: u32,
    ) -> (
        impl IntoIterator<Item = PacketEntitiesMessage>,
        PacketEntitiesMessage,
        PacketEntitiesMessage,
    ) {
        // baselines in reverse order
        let mut baselines = [
            encode_entities(
                state.instance_baselines[1]
                    .clone()
                    .into_values()
                    .collect::<Vec<_>>(),
                self.max_entities,
                None,
                Vec::new(),
            ),
            encode_entities(
                state.instance_baselines[0]
                    .clone()
                    .into_values()
                    .collect::<Vec<_>>(),
                self.max_entities,
                None,
                Vec::new(),
            ),
        ];
        for entity in self.entities.values_mut() {
            match state.instance_baselines[0].get(entity.entity_index) {
                Some(baseline_entity) if baseline_entity.server_class == entity.server_class => {
                    entity.update_type = UpdateType::Preserve;
                }
                Some(_baseline_entity) => {
                    // encode the baseline if the baseline server class differs
                    let props = take(&mut entity.props);
                    entity.props = take(&mut entity.baseline_props);
                    entity.apply_update(&props);
                    entity.update_type = UpdateType::Enter;
                }
                None => {
                    entity.update_type = UpdateType::Enter;
                }
            }
        }

        // create deletes for all entities that have an updated baseline but are since removed
        let removed_entities = self
            .baseline_ids(state)
            .into_iter()
            .filter(|id| !self.entities.contains_key(id))
            .collect::<Vec<_>>()
            .into_iter();

        let entities = encode_entities(
            self.entities
                .into_values()
                .chain(removed_entities.map(|removed| PacketEntity {
                    server_class: ClassId::from(0),
                    entity_index: removed,
                    baseline_props: vec![],
                    props: vec![],
                    in_pvs: false,
                    update_type: if self.deleted_entities.contains(&removed) {
                        UpdateType::Delete
                    } else {
                        UpdateType::Leave
                    },
                    serial_number: 0,
                    delay: None,
                }))
                .collect::<Vec<_>>(),
            self.max_entities,
            Some(delta),
            Vec::new(),
        );

        baselines[0].updated_base_line = true;
        baselines[1].updated_base_line = true;
        baselines[0].base_line = 1;

        (
            baselines.into_iter(),
            entities,
            encode_entities(
                Vec::new(),
                self.max_entities,
                Some(delta + 1),
                self.removed_entities.into_iter().collect(),
            ),
        )
    }
}

fn encode_entities(
    mut entities: Vec<PacketEntity>,
    max_entries: u16,
    delta: Option<u32>,
    removed_entities: Vec<EntityId>,
) -> PacketEntitiesMessage {
    entities.sort_by(|a, b| a.entity_index.cmp(&b.entity_index));
    PacketEntitiesMessage {
        entities,
        removed_entities,
        max_entries,
        delta: delta.and_then(|delta| NonZeroU32::new(delta)),
        base_line: 0,
        updated_base_line: false,
    }
}
