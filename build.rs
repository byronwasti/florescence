fn main() {
    let greeter_service = tonic_build::manual::Service::builder()
        .name("Gossip")
        .package("bincode.gossip")
        .method(
            tonic_build::manual::Method::builder()
                .name("gossip")
                .route_name("Gossip")
                .input_type("crate::gossip::GossipRequest")
                .output_type("crate::gossip::GossipResponse")
                .codec_path("crate::codec::BinCodec")
                .client_streaming()
                .server_streaming()
                .build(),
        )
        .build();

    tonic_build::manual::Builder::new().compile(&[greeter_service]);
}
