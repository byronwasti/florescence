fn main() {
    let greeter_service = tonic_build::manual::Service::builder()
        .name("Gossip")
        .package("bincode.gossip")
        .method(
            tonic_build::manual::Method::builder()
                .name("gossip")
                .route_name("Gossip")
                .input_type("crate::engine::tonic_engine::rpc::TonicReqWrapper")
                .output_type("crate::engine::tonic_engine::rpc::TonicReqWrapper")
                .codec_path("crate::engine::tonic_engine::codec::BinCodec")
                .client_streaming()
                .server_streaming()
                .build(),
        )
        .build();

    tonic_build::manual::Builder::new().compile(&[greeter_service]);
}
