//! Tests for acl_applicator.rs — ACL event handling and helper functions.

use privstack_sync::acl_applicator::{is_acl_event, AclApplicator};
use privstack_sync::policy::{EnterpriseSyncPolicy, SyncRole, TeamId};
use privstack_sync::AclEventHandler;
use privstack_types::{EntityId, Event, EventPayload, HybridTimestamp, PeerId};
use std::sync::Arc;

fn make_acl_event(entity_id: EntityId, payload: EventPayload) -> Event {
    Event::new(entity_id, PeerId::new(), HybridTimestamp::now(), payload)
}

// ── is_acl_event ────────────────────────────────────────────────

#[test]
fn is_acl_event_grant_peer() {
    let payload = EventPayload::AclGrantPeer {
        entity_id: EntityId::new().to_string(),
        peer_id: PeerId::new().to_string(),
        role: "Editor".to_string(),
    };
    assert!(is_acl_event(&payload));
}

#[test]
fn is_acl_event_revoke_peer() {
    let payload = EventPayload::AclRevokePeer {
        entity_id: EntityId::new().to_string(),
        peer_id: PeerId::new().to_string(),
    };
    assert!(is_acl_event(&payload));
}

#[test]
fn is_acl_event_grant_team() {
    let payload = EventPayload::AclGrantTeam {
        entity_id: EntityId::new().to_string(),
        team_id: uuid::Uuid::new_v4().to_string(),
        role: "Viewer".to_string(),
    };
    assert!(is_acl_event(&payload));
}

#[test]
fn is_acl_event_revoke_team() {
    let payload = EventPayload::AclRevokeTeam {
        entity_id: EntityId::new().to_string(),
        team_id: uuid::Uuid::new_v4().to_string(),
    };
    assert!(is_acl_event(&payload));
}

#[test]
fn is_acl_event_set_default() {
    let payload = EventPayload::AclSetDefault {
        entity_id: EntityId::new().to_string(),
        role: Some("Admin".to_string()),
    };
    assert!(is_acl_event(&payload));
}

#[test]
fn is_acl_event_team_add_peer() {
    let payload = EventPayload::TeamAddPeer {
        team_id: uuid::Uuid::new_v4().to_string(),
        peer_id: PeerId::new().to_string(),
    };
    assert!(is_acl_event(&payload));
}

#[test]
fn is_acl_event_team_remove_peer() {
    let payload = EventPayload::TeamRemovePeer {
        team_id: uuid::Uuid::new_v4().to_string(),
        peer_id: PeerId::new().to_string(),
    };
    assert!(is_acl_event(&payload));
}

#[test]
fn is_acl_event_false_for_full_snapshot() {
    let payload = EventPayload::FullSnapshot {
        entity_type: "note".to_string(),
        json_data: "{}".to_string(),
    };
    assert!(!is_acl_event(&payload));
}

#[test]
fn is_acl_event_false_for_entity_created() {
    let payload = EventPayload::EntityCreated {
        entity_type: "note".to_string(),
        json_data: "{}".to_string(),
    };
    assert!(!is_acl_event(&payload));
}

#[test]
fn is_acl_event_false_for_entity_updated() {
    let payload = EventPayload::EntityUpdated {
        entity_type: "note".to_string(),
        json_data: "{}".to_string(),
    };
    assert!(!is_acl_event(&payload));
}

#[test]
fn is_acl_event_false_for_entity_deleted() {
    let payload = EventPayload::EntityDeleted {
        entity_type: "note".to_string(),
    };
    assert!(!is_acl_event(&payload));
}

// ── AclApplicator: handle_acl_event ─────────────────────────────

#[tokio::test]
async fn handle_acl_grant_peer() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy.clone());

    let entity = EntityId::new();
    let peer = PeerId::new();
    let event = make_acl_event(
        entity,
        EventPayload::AclGrantPeer {
            entity_id: entity.to_string(),
            peer_id: peer.to_string(),
            role: "Editor".to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.is_ok());
    assert!(result.unwrap());

    let role = policy.resolve_role(&peer, &entity).await;
    assert_eq!(role, Some(SyncRole::Editor));
}

#[tokio::test]
async fn handle_acl_revoke_peer() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy.clone());

    let entity = EntityId::new();
    let peer = PeerId::new();

    // Grant first
    policy.grant_peer_role(entity, peer, SyncRole::Editor).await;
    assert_eq!(policy.resolve_role(&peer, &entity).await, Some(SyncRole::Editor));

    // Revoke via event
    let event = make_acl_event(
        entity,
        EventPayload::AclRevokePeer {
            entity_id: entity.to_string(),
            peer_id: peer.to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.unwrap());
    assert_eq!(policy.resolve_role(&peer, &entity).await, None);
}

#[tokio::test]
async fn handle_acl_grant_team() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy.clone());

    let entity = EntityId::new();
    let team = TeamId::new();
    let event = make_acl_event(
        entity,
        EventPayload::AclGrantTeam {
            entity_id: entity.to_string(),
            team_id: team.0.to_string(),
            role: "Admin".to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.unwrap());

    let acls = policy.acls.read().await;
    let acl = acls.get(&entity).unwrap();
    assert_eq!(acl.team_roles.get(&team), Some(&SyncRole::Admin));
}

#[tokio::test]
async fn handle_acl_revoke_team() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy.clone());

    let entity = EntityId::new();
    let team = TeamId::new();
    policy.grant_team_role(entity, team, SyncRole::Editor).await;

    let event = make_acl_event(
        entity,
        EventPayload::AclRevokeTeam {
            entity_id: entity.to_string(),
            team_id: team.0.to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.unwrap());

    let acls = policy.acls.read().await;
    if let Some(acl) = acls.get(&entity) {
        assert!(acl.team_roles.get(&team).is_none());
    }
}

#[tokio::test]
async fn handle_acl_set_default_with_role() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy.clone());

    let entity = EntityId::new();
    let event = make_acl_event(
        entity,
        EventPayload::AclSetDefault {
            entity_id: entity.to_string(),
            role: Some("Viewer".to_string()),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.unwrap());

    let acls = policy.acls.read().await;
    let acl = acls.get(&entity).unwrap();
    assert_eq!(acl.default_role, Some(SyncRole::Viewer));
}

#[tokio::test]
async fn handle_acl_set_default_with_none_role() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy.clone());

    let entity = EntityId::new();
    // Set a default first
    policy.set_default_role(entity, Some(SyncRole::Editor)).await;

    // Clear via None
    let event = make_acl_event(
        entity,
        EventPayload::AclSetDefault {
            entity_id: entity.to_string(),
            role: None,
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.unwrap());

    let acls = policy.acls.read().await;
    let acl = acls.get(&entity).unwrap();
    assert_eq!(acl.default_role, None);
}

#[tokio::test]
async fn handle_acl_set_default_with_empty_string() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy.clone());

    let entity = EntityId::new();
    policy.set_default_role(entity, Some(SyncRole::Admin)).await;

    // Clear via empty string
    let event = make_acl_event(
        entity,
        EventPayload::AclSetDefault {
            entity_id: entity.to_string(),
            role: Some(String::new()),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.unwrap());

    let acls = policy.acls.read().await;
    let acl = acls.get(&entity).unwrap();
    assert_eq!(acl.default_role, None);
}

#[tokio::test]
async fn handle_team_add_peer() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy.clone());

    let team = TeamId::new();
    let peer = PeerId::new();
    let event = make_acl_event(
        EntityId::new(),
        EventPayload::TeamAddPeer {
            team_id: team.0.to_string(),
            peer_id: peer.to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.unwrap());

    let teams = policy.teams.read().await;
    assert!(teams.get(&team).unwrap().contains(&peer));
}

#[tokio::test]
async fn handle_team_remove_peer() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy.clone());

    let team = TeamId::new();
    let peer = PeerId::new();
    policy.add_team_member(team, peer).await;

    let event = make_acl_event(
        EntityId::new(),
        EventPayload::TeamRemovePeer {
            team_id: team.0.to_string(),
            peer_id: peer.to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.unwrap());

    let teams = policy.teams.read().await;
    let members = teams.get(&team);
    assert!(members.is_none() || !members.unwrap().contains(&peer));
}

#[tokio::test]
async fn handle_non_acl_event_returns_false() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy);

    let event = make_acl_event(
        EntityId::new(),
        EventPayload::FullSnapshot {
            entity_type: "note".to_string(),
            json_data: "{}".to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.is_ok());
    assert!(!result.unwrap());
}

// ── parse_role errors ───────────────────────────────────────────

#[tokio::test]
async fn invalid_role_string_returns_error() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy);

    let entity = EntityId::new();
    let event = make_acl_event(
        entity,
        EventPayload::AclGrantPeer {
            entity_id: entity.to_string(),
            peer_id: PeerId::new().to_string(),
            role: "SuperUser".to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.is_err());
}

// ── parse_team_id errors ────────────────────────────────────────

#[tokio::test]
async fn invalid_team_id_returns_error() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy);

    let entity = EntityId::new();
    let event = make_acl_event(
        entity,
        EventPayload::AclGrantTeam {
            entity_id: entity.to_string(),
            team_id: "not-a-uuid".to_string(),
            role: "Editor".to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn invalid_team_id_in_team_add_peer_returns_error() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy);

    let event = make_acl_event(
        EntityId::new(),
        EventPayload::TeamAddPeer {
            team_id: "bad-uuid".to_string(),
            peer_id: PeerId::new().to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn invalid_peer_id_in_grant_returns_error() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy);

    let entity = EntityId::new();
    let event = make_acl_event(
        entity,
        EventPayload::AclGrantPeer {
            entity_id: entity.to_string(),
            peer_id: "not-a-uuid".to_string(),
            role: "Editor".to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn invalid_entity_id_in_grant_returns_error() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy);

    let event = make_acl_event(
        EntityId::new(),
        EventPayload::AclGrantPeer {
            entity_id: "not-a-uuid".to_string(),
            peer_id: PeerId::new().to_string(),
            role: "Editor".to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.is_err());
}

// ── Additional coverage: parse_role "Owner" variant ─────────────

#[tokio::test]
async fn handle_acl_grant_peer_owner_role() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy.clone());

    let entity = EntityId::new();
    let peer = PeerId::new();
    let event = make_acl_event(
        entity,
        EventPayload::AclGrantPeer {
            entity_id: entity.to_string(),
            peer_id: peer.to_string(),
            role: "Owner".to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.unwrap());
    let role = policy.resolve_role(&peer, &entity).await;
    assert_eq!(role, Some(SyncRole::Owner));
}

#[tokio::test]
async fn handle_acl_grant_peer_viewer_role() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy.clone());

    let entity = EntityId::new();
    let peer = PeerId::new();
    let event = make_acl_event(
        entity,
        EventPayload::AclGrantPeer {
            entity_id: entity.to_string(),
            peer_id: peer.to_string(),
            role: "Viewer".to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.unwrap());
    let role = policy.resolve_role(&peer, &entity).await;
    assert_eq!(role, Some(SyncRole::Viewer));
}

// ── Error paths for invalid IDs in revoke/team operations ───────

#[tokio::test]
async fn invalid_entity_id_in_revoke_peer_returns_error() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy);

    let event = make_acl_event(
        EntityId::new(),
        EventPayload::AclRevokePeer {
            entity_id: "not-a-uuid".to_string(),
            peer_id: PeerId::new().to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn invalid_peer_id_in_revoke_peer_returns_error() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy);

    let entity = EntityId::new();
    let event = make_acl_event(
        entity,
        EventPayload::AclRevokePeer {
            entity_id: entity.to_string(),
            peer_id: "not-a-uuid".to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn invalid_entity_id_in_grant_team_returns_error() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy);

    let event = make_acl_event(
        EntityId::new(),
        EventPayload::AclGrantTeam {
            entity_id: "not-a-uuid".to_string(),
            team_id: uuid::Uuid::new_v4().to_string(),
            role: "Editor".to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn invalid_entity_id_in_revoke_team_returns_error() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy);

    let event = make_acl_event(
        EntityId::new(),
        EventPayload::AclRevokeTeam {
            entity_id: "not-a-uuid".to_string(),
            team_id: uuid::Uuid::new_v4().to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn invalid_team_id_in_revoke_team_returns_error() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy);

    let entity = EntityId::new();
    let event = make_acl_event(
        entity,
        EventPayload::AclRevokeTeam {
            entity_id: entity.to_string(),
            team_id: "bad-uuid".to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn invalid_team_id_in_team_remove_peer_returns_error() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy);

    let event = make_acl_event(
        EntityId::new(),
        EventPayload::TeamRemovePeer {
            team_id: "bad-uuid".to_string(),
            peer_id: PeerId::new().to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn invalid_peer_id_in_team_add_peer_returns_error() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy);

    let event = make_acl_event(
        EntityId::new(),
        EventPayload::TeamAddPeer {
            team_id: uuid::Uuid::new_v4().to_string(),
            peer_id: "bad-uuid".to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn invalid_peer_id_in_team_remove_peer_returns_error() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy);

    let event = make_acl_event(
        EntityId::new(),
        EventPayload::TeamRemovePeer {
            team_id: uuid::Uuid::new_v4().to_string(),
            peer_id: "bad-uuid".to_string(),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn invalid_entity_id_in_set_default_returns_error() {
    let policy = Arc::new(EnterpriseSyncPolicy::new());
    let applicator = AclApplicator::new(policy);

    let event = make_acl_event(
        EntityId::new(),
        EventPayload::AclSetDefault {
            entity_id: "not-a-uuid".to_string(),
            role: Some("Viewer".to_string()),
        },
    );

    let result = applicator.handle_acl_event(&event).await;
    assert!(result.is_err());
}
