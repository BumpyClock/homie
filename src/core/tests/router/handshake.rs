use super::*;

#[tokio::test]
async fn handshake_includes_cron_service() {
    let addr = start_server(ServerConfig::default()).await;
    let url = format!("ws://{addr}/ws");
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();

    let hello = serde_json::to_string(&ClientHello {
        protocol: VersionRange::new(1, 1),
        client_id: "test-client/0.1.0".into(),
        auth_token: None,
        capabilities: vec![],
    })
    .unwrap();
    ws.send(text_msg(hello)).await.unwrap();

    let text = next_text(&mut ws).await;
    let resp: HandshakeResponse = serde_json::from_str(&text).unwrap();
    let services = match resp {
        HandshakeResponse::Hello(h) => h.services,
        _ => panic!("expected server hello"),
    };
    assert!(services.iter().any(|service| service.service == "cron"));
}

#[tokio::test]
async fn handshake_advertises_registered_services() {
    let addr = start_server(ServerConfig::default()).await;
    let url = format!("ws://{addr}/ws");
    let (mut stream, _) = tokio_tungstenite::connect_async(&url).await.unwrap();

    let hello = serde_json::to_string(&ClientHello {
        protocol: VersionRange::new(1, 1),
        client_id: "test-client/0.1.0".into(),
        auth_token: None,
        capabilities: vec![],
    })
    .unwrap();

    stream
        .send(tungstenite::Message::Text(hello.into()))
        .await
        .unwrap();

    let t = next_text(&mut stream).await;
    let resp: HandshakeResponse = serde_json::from_str(&t).unwrap();

    match resp {
        HandshakeResponse::Hello(hello) => {
            assert!(!hello.services.is_empty());
            assert_eq!(hello.services[0].service, "terminal");
            assert_eq!(hello.services[0].version, "1.0");
        }
        HandshakeResponse::Reject(r) => panic!("unexpected reject: {r:?}"),
    }
}
