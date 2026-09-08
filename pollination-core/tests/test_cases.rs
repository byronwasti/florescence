use pollination::core::PollinationCore;
use tracing::info;

#[test]
fn test_case_1() {
    tracing_subscriber::fmt().with_test_writer().try_init();
    let mut n1: PollinationCore<u64> =
        serde_json::from_str(include_str!("tc1_dump1.json")).expect("Unparseable");
    let mut n2: PollinationCore<u64> =
        serde_json::from_str(include_str!("tc1_dump2.json")).expect("Unparseable");

    info!("------ 0 ------");
    let m = n1.heartbeat_message();

    info!("------ 1 ------");
    let m = n2.handle_message(m).expect("Some response");
    info!("{m}");

    info!("------ 2 ------");
    let m = n1.handle_message(m).expect("Some response");
    info!("{m}");

    info!("------ 3 ------");
    let m = n2.handle_message(m).expect("Some response");
    info!("{m}");

    info!("------ 4 ------");
    let m = n1.handle_message(m);
    info!("{m:?}");

    info!("n1: {n1}");
    info!("n2: {n2}");
}

#[test]
fn test_case_2() {
    tracing_subscriber::fmt().with_test_writer().try_init();
    let mut n1: PollinationCore<u64> =
        serde_json::from_str(include_str!("tc1_dump1.json")).expect("Unparseable");
    let mut n2: PollinationCore<u64> =
        serde_json::from_str(include_str!("tc1_dump2.json")).expect("Unparseable");

    info!("------ 0 ------");
    info!("n1: {n1}");
    info!("n2: {n2}");
    let m = n2.heartbeat_message();

    info!("------ 1 ------");
    let m = n1.handle_message(m).expect("Some response");
    info!("{m}");

    info!("------ 2 ------");
    let m = n2.handle_message(m).expect("Some response");
    info!("{m}");

    info!("------ 3 ------");
    let m = n1.handle_message(m);
    info!("{m:?}");

    info!("n1: {n1}");
    info!("n2: {n2}");
}
