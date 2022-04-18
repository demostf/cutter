use std::collections::BTreeMap;
use tf_demo_parser::demo::message::stringtable::{StringTableMeta, UpdateStringTableMessage};
use tf_demo_parser::demo::message::Message;
use tf_demo_parser::demo::packet::stringtable::StringTableEntry;

#[derive(Default)]
pub struct StringTable {
    entries: BTreeMap<u16, StringTableEntry<'static>>,
}

#[derive(Default)]
pub struct StringTablesUpdates {
    pub tables: BTreeMap<u8, StringTable>,
}

impl StringTablesUpdates {
    pub fn handle_message(&mut self, message: &Message) {
        match message {
            Message::UpdateStringTable(msg) => {
                let table = self.tables.entry(msg.table_id).or_default();
                for (id, entry) in &msg.entries {
                    table.entries.insert(*id, entry.to_owned());
                }
            }
            _ => {}
        }
    }

    pub fn encode(self) -> impl IntoIterator<Item = UpdateStringTableMessage<'static>> {
        self.tables
            .into_iter()
            .map(|(table_id, table)| UpdateStringTableMessage {
                entries: table.entries.into_iter().collect(),
                table_id,
            })
    }
}
