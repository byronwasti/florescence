use pollination::core::{PollinationCore, PollinationMessage};
use tracing::info;

#[test]
fn test_case_1() {
    tracing_subscriber::fmt().with_test_writer().try_init();
    let mut n: PollinationCore<u64> =
        serde_json::from_str(include_str!("tc2_core.json")).expect("Unparseable");
    let n0 = n.clone();

    let mut m: PollinationMessage<u64> =
        serde_json::from_str(include_str!("tc2_msg.json")).expect("Unparseable");

    let msg = n.handle_message(m).expect("Message");

    info!("n_start: {n0}");
    info!("n_end: {n}");
    info!("msg_out: {msg}");

    let t0 = n0.timestamp();
    let t = n.timestamp();
    info!("t - t0 = {}", t.clone().diff(t0));
    info!("t0 - t1 = {}", t0.clone().diff(t));
}
