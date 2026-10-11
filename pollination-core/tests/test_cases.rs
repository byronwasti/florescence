use pollination::core::{PollinationCore, PollinationMessage};
use tracing::info;

#[test]
fn test_case_0() {
    tracing_subscriber::fmt().with_test_writer().try_init();
    let mut n: PollinationCore<usize> = serde_json::from_str(r#"{"id":{"SubTree":[{"SubTree":["Zero",{"SubTree":["Zero","One"]}]},"Zero"]},"core_map":{"timestamp":{"SubTree":[3,{"SubTree":[0,{"Leaf":0},{"Leaf":3}]},{"Leaf":1}]},"data":[[{"SubTree":["Zero",{"SubTree":["Zero","One"]}]},{"uuid":"aa408481-e557-77e7-8c72-22b1ab19ecce","addr":2,"timestamp":2,"status":"Healthy"}],[{"SubTree":[{"SubTree":["Zero",{"SubTree":["Zero","One"]}]},"Zero"]},{"uuid":"524a84ae-f1be-171a-dfa0-f98e8882f6f4","addr":4,"timestamp":4,"status":"Healthy"}],[{"SubTree":[{"SubTree":["One","Zero"]},"Zero"]},{"uuid":"45e072f1-296c-7145-9079-9033050894c3","addr":0,"timestamp":3,"status":"Healthy"}],[{"SubTree":["Zero",{"SubTree":["One","Zero"]}]},{"uuid":"6a1bfe15-a6ba-3c54-314e-37aa82a2efeb","addr":3,"timestamp":4,"status":"Healthy"}],[{"SubTree":[{"SubTree":["Zero",{"SubTree":[{"SubTree":["Zero","One"]},"Zero"]}]},"Zero"]},{"uuid":"b9a62a65-9719-25b8-4ccd-6a417165eaf8","addr":1,"timestamp":1,"status":"Healthy"}],[{"SubTree":[{"SubTree":["Zero",{"SubTree":[{"SubTree":["One","Zero"]},"Zero"]}]},"Zero"]},{"uuid":"aa408481-e557-77e7-8c72-22b1ab19ecce","addr":2,"timestamp":5,"status":"Healthy"}]],"index":{"SubTree":[{"SubTree":[{"Leaf":2},{"SubTree":[{"SubTree":[{"Leaf":5},{"Leaf":4}]},{"Leaf":1}]}]},{"SubTree":[{"Leaf":3},{"Leaf":0}]}]}},"origin_hash":12457498158043442052,"own_info":{"uuid":"524a84ae-f1be-171a-dfa0-f98e8882f6f4","addr":4,"timestamp":4,"status":"Healthy"}}"#).unwrap();

    let m: PollinationMessage<usize> = serde_json::from_str(r#"{"uuid":"b9a62a65-9719-25b8-4ccd-6a417165eaf8","id":{"SubTree":[{"SubTree":[{"SubTree":[{"SubTree":["One","Zero"]},"Zero"]},"Zero"]},"Zero"]},"timestamp":{"SubTree":[3,{"SubTree":[1,{"SubTree":[0,{"SubTree":[0,{"Leaf":1},{"Leaf":0}]},{"Leaf":0}]},{"Leaf":0}]},{"Leaf":0}]},"origin_hash":12457498158043442052,"membership_hash":11470857683576538083,"unique_count":5,"patch":{"timestamp":{"SubTree":[0,{"SubTree":[0,{"SubTree":[4,{"SubTree":[0,{"Leaf":1},{"Leaf":0}]},{"Leaf":0}]},{"Leaf":0}]},{"Leaf":0}]},"inner":[[{"SubTree":[{"SubTree":[{"SubTree":[{"SubTree":["One","Zero"]},"Zero"]},"Zero"]},"Zero"]},{"uuid":"b9a62a65-9719-25b8-4ccd-6a417165eaf8","addr":1,"timestamp":4,"status":"Healthy"}],[{"SubTree":[{"SubTree":[{"SubTree":["Zero","One"]},"Zero"]},"Zero"]},{"uuid":"45e072f1-296c-7145-9079-9033050894c3","addr":0,"timestamp":4,"status":"Healthy"}],[{"SubTree":[{"SubTree":[{"SubTree":[{"SubTree":["Zero","One"]},"Zero"]},"Zero"]},"Zero"]},{"uuid":"524a84ae-f1be-171a-dfa0-f98e8882f6f4","addr":4,"timestamp":3,"status":"Healthy"}]]},"new_membership":false,"full_patch":false}"#).unwrap();

    let res = n.handle_message(m);
}

#[test]
#[ignore]
fn test_case_1() {
    tracing_subscriber::fmt().with_test_writer().try_init();
    let mut n: PollinationCore<u64> =
        serde_json::from_str(include_str!("tc2_core.json")).expect("Unparseable");
    let n0 = n.clone();

    let mut m: PollinationMessage<u64> =
        serde_json::from_str(include_str!("tc2_msg.json")).expect("Unparseable");

    info!("n_start: {n0}");
    info!("m_in: {m}");

    let msg = n.handle_message(m).expect("Message");

    info!("n_end: {n}");
    info!("msg_out: {msg}");

    let t0 = n0.timestamp();
    let t = n.timestamp();
    info!("t - t0 = {}", t.clone().diff(t0));
    info!("t0 - t1 = {}", t0.clone().diff(t));
}
