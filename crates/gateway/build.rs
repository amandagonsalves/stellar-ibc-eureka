fn main() {
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());

    prost_build::Config::new()
        .file_descriptor_set_path(out_dir.join("stellar_gateway_descriptor.bin"))
        .compile_protos(&["proto/stellar_gateway.proto"], &["proto/"])
        .unwrap();

    let codec = "tonic_prost::ProstCodec";

    let query_methods = vec![
        (
            "latest_height",
            "LatestHeight",
            "crate::proto::LatestHeightRequest",
            "crate::proto::LatestHeightResponse",
        ),
        (
            "query_client_state",
            "QueryClientState",
            "crate::proto::QueryClientStateRequest",
            "crate::proto::QueryClientStateResponse",
        ),
        (
            "query_consensus_state",
            "QueryConsensusState",
            "crate::proto::QueryConsensusStateRequest",
            "crate::proto::QueryConsensusStateResponse",
        ),
        (
            "query_packet_commitment",
            "QueryPacketCommitment",
            "crate::proto::QueryPacketCommitmentRequest",
            "crate::proto::QueryPacketCommitmentResponse",
        ),
        (
            "query_packet_receipt",
            "QueryPacketReceipt",
            "crate::proto::QueryPacketReceiptRequest",
            "crate::proto::QueryPacketReceiptResponse",
        ),
        (
            "query_acknowledgement",
            "QueryAcknowledgement",
            "crate::proto::QueryAcknowledgementRequest",
            "crate::proto::QueryAcknowledgementResponse",
        ),
        (
            "query_next_seq_recv",
            "QueryNextSeqRecv",
            "crate::proto::QueryNextSeqRecvRequest",
            "crate::proto::QueryNextSeqRecvResponse",
        ),
        (
            "query_ibc_header",
            "QueryIbcHeader",
            "crate::proto::QueryIbcHeaderRequest",
            "crate::proto::QueryIbcHeaderResponse",
        ),
    ];

    let msg_methods = vec![
        (
            "submit_signed_tx",
            "SubmitSignedTx",
            "crate::proto::SubmitSignedTxRequest",
            "crate::proto::SubmitSignedTxResponse",
        ),
        (
            "create_client",
            "CreateClient",
            "crate::proto::MsgCreateClientRequest",
            "crate::proto::MsgCreateClientResponse",
        ),
        (
            "update_client",
            "UpdateClient",
            "crate::proto::MsgUpdateClientRequest",
            "crate::proto::MsgUpdateClientResponse",
        ),
        (
            "register_counterparty",
            "RegisterCounterparty",
            "crate::proto::MsgRegisterCounterpartyRequest",
            "crate::proto::MsgRegisterCounterpartyResponse",
        ),
        (
            "recv_packet",
            "RecvPacket",
            "crate::proto::MsgRecvPacketRequest",
            "crate::proto::MsgRecvPacketResponse",
        ),
        (
            "ack_packet",
            "AckPacket",
            "crate::proto::MsgAckPacketRequest",
            "crate::proto::MsgAckPacketResponse",
        ),
        (
            "timeout_packet",
            "TimeoutPacket",
            "crate::proto::MsgTimeoutPacketRequest",
            "crate::proto::MsgTimeoutPacketResponse",
        ),
    ];

    let mut query_svc = tonic_build::manual::Service::builder()
        .name("StellarGatewayQuery")
        .package("stellar.gateway.v1");

    for (name, route, input, output) in &query_methods {
        query_svc = query_svc.method(
            tonic_build::manual::Method::builder()
                .name(name)
                .route_name(route)
                .input_type(input)
                .output_type(output)
                .codec_path(codec)
                .build(),
        );
    }

    let mut msg_svc = tonic_build::manual::Service::builder()
        .name("StellarGatewayMsg")
        .package("stellar.gateway.v1");
    for (name, route, input, output) in &msg_methods {
        msg_svc = msg_svc.method(
            tonic_build::manual::Method::builder()
                .name(name)
                .route_name(route)
                .input_type(input)
                .output_type(output)
                .codec_path(codec)
                .build(),
        );
    }

    tonic_build::manual::Builder::new()
        .build_client(false)
        .compile(&[query_svc.build(), msg_svc.build()]);
}
