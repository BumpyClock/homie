use super::*;

#[test]
fn upsert_and_get_pairing() {
    let store = make_store();
    let now = now_unix();
    let pairing = PairingRecord {
        pairing_id: "p1".into(),
        nonce: "n1".into(),
        status: PairingStatus::Pending,
        created_at: now,
        expires_at: now + 60,
        approved_by: None,
    };
    store.upsert_pairing(&pairing).unwrap();

    let loaded = store.get_pairing("p1").unwrap().unwrap();
    assert_eq!(loaded.status, PairingStatus::Pending);
}

#[test]
fn prune_pairings_expires_and_deletes() {
    let store = make_store();
    let now = now_unix();
    let expired = PairingRecord {
        pairing_id: "old".into(),
        nonce: "n".into(),
        status: PairingStatus::Pending,
        created_at: now,
        expires_at: now.saturating_sub(10),
        approved_by: None,
    };
    let deleted = PairingRecord {
        pairing_id: "gone".into(),
        nonce: "n2".into(),
        status: PairingStatus::Pending,
        created_at: now,
        expires_at: now.saturating_sub(10_000),
        approved_by: None,
    };
    store.upsert_pairing(&expired).unwrap();
    store.upsert_pairing(&deleted).unwrap();

    store.prune_pairings(3600).unwrap();
    let pairings = store.list_pairings().unwrap();
    assert_eq!(pairings.len(), 1);
    assert_eq!(pairings[0].pairing_id, "old");
    assert_eq!(pairings[0].status, PairingStatus::Expired);
}
