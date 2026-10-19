use pollination::core::{PollinationCore, PollinationMessage};
use tracing::info;

#[test]
fn test_case_1() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .try_init();
    let mut n: PollinationCore<u64> =
        serde_json::from_str(include_str!("tc2_node.json")).expect("Unparseable");
    let n0 = n.clone();

    let mut m: PollinationMessage<u64> =
        serde_json::from_str(include_str!("tc2_msg.json")).expect("Unparseable");

    info!("{}", n.id());

    n.recycle();

    info!("{}", n.id());

    let msg = n.handle_message(m).expect("Message");

    assert!(n.core_map().is_complete());

    let t0 = n0.timestamp();
    let t = n.timestamp();
}
