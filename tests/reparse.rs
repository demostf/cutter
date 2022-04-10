use democutter::cut;
use pretty_assertions::assert_eq;
use std::fs;
use tf_demo_parser::demo::message::packetentities::EntityId;
use tf_demo_parser::demo::message::Message;
use tf_demo_parser::demo::packet::message::MessagePacketMeta;
use tf_demo_parser::demo::parser::gamestateanalyser::GameStateAnalyser;
use tf_demo_parser::demo::parser::handler::BorrowMessageHandler;
use tf_demo_parser::demo::parser::MessageHandler;
use tf_demo_parser::demo::sendprop::SendProp;
use tf_demo_parser::{Demo, DemoParser, MessageType, ParserState};

fn test_reparse_with_analyser<A: BorrowMessageHandler + Default, F: Fn(&A::Output, &A::Output)>(
    f: F,
) {
    let file = fs::read("test_data/gully.dem").unwrap();
    let output = cut(&file, 30000, 50000);

    let original = Demo::new(&file);
    let cut = Demo::new(&output);

    let original_parser = DemoParser::new_with_analyser(original.get_stream(), A::default());
    let cut_parser = DemoParser::new_with_analyser(cut.get_stream(), A::default());

    let mut original_ticks = original_parser.ticker().unwrap().1;
    let mut cut_ticks = cut_parser.ticker().unwrap().1;

    while let Some(tick) = original_ticks.next().unwrap() {
        if tick.tick > 30010 && tick.tick < 50000 {
            break;
        }
    }

    while let Some(tick) = cut_ticks.next().unwrap() {
        if tick.tick > 10 && tick.tick < 20000 {
            break;
        }
    }

    original_ticks
        .next()
        .unwrap()
        .expect("no more ticks in original");
    cut_ticks.next().unwrap().expect("no more ticks in cut");

    while let (Some(original_tick), Some(cut_tick)) =
        (original_ticks.next().unwrap(), cut_ticks.next().unwrap())
    {
        assert_eq!(original_tick.tick, cut_tick.tick + 30000);
        let original_state = &original_tick.state;
        let cut_state = &cut_tick.state;

        f(original_state, cut_state);
    }
}

#[derive(Default)]
struct EntityDumper {
    entities: Vec<(EntityId, Vec<SendProp>)>,
}

impl MessageHandler for EntityDumper {
    type Output = Vec<(EntityId, Vec<SendProp>)>;

    fn does_handle(message_type: MessageType) -> bool {
        match message_type {
            MessageType::PacketEntities => true,
            _ => false,
        }
    }

    fn handle_message(&mut self, message: &Message, _tick: u32) {
        match message {
            Message::PacketEntities(entity_message) => {
                for entity in &entity_message.entities {
                    self.entities
                        .push((entity.entity_index, entity.props().cloned().collect()));
                }
            }
            _ => {}
        }
    }

    fn handle_packet_meta(&mut self, _tick: u32, _meta: &MessagePacketMeta) {
        self.entities.clear();
    }

    fn into_output(self, _state: &ParserState) -> Self::Output {
        self.entities
    }
}

impl BorrowMessageHandler for EntityDumper {
    fn borrow_output(&self, _state: &ParserState) -> &Self::Output {
        &self.entities
    }
}

#[test]
fn test_reparse_game_state() {
    test_reparse_with_analyser::<GameStateAnalyser, _>(|original_state, cut_state| {
        assert_eq!(original_state.world, cut_state.world);
        assert_eq!(original_state.players, cut_state.players);
        assert_eq!(original_state.buildings, cut_state.buildings);
    })
}

// #[test]
// fn test_reparse_entities() {
//     test_reparse_with_analyser::<EntityDumper, _>(|original_state, cut_state| {
//         assert_eq!(original_state.len(), cut_state.len());
//         panic!();
//     })
// }
