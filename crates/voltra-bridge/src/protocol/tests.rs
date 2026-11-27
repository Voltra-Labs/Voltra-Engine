//! A request and a response through their wire form.

use super::*;

#[test]
fn a_command_reads_back_from_its_line() {
    let request = Request {
        id: 7,
        command: Command::ComponentSet {
            entity: "Player".into(),
            component: "Sprite2D".into(),
            ron: "(color: (1.0, 0.0, 0.0, 1.0))".into(),
        },
    };
    let line = serde_json::to_string(&request).expect("writes");
    assert!(line.contains("\"op\":\"component_set\""), "{line}");
    assert_eq!(
        serde_json::from_str::<Request>(&line).expect("reads"),
        request
    );
    let response = Response::answer(7, Err("no such entity".into()));
    let back: Response =
        serde_json::from_str(&serde_json::to_string(&response).expect("writes")).expect("reads");
    assert_eq!(back.into_result(), Err("no such entity".to_owned()));
}
