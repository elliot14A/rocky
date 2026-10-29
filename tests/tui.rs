use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use rocky::tui::load::load_fleet;
use rocky::tui::run::{apply_action, classify_key_event, ActionEffect, KeyAction};
use rocky::tui::state::{ActivePopup, FocusPanel, ServiceForm, ServiceKind, TuiState};

#[tokio::test]
async fn test_tui_load_fleet_and_state_initialization() {
    let fleet_root = Path::new("examples/supabase");
    let fleet = load_fleet(fleet_root)
        .await
        .expect("failed to load supabase example fleet");

    assert_eq!(fleet.environments.len(), 1);
    let prod = &fleet.environments[0];
    assert_eq!(prod.name, "prod");
    assert!(!prod.domain.is_empty());
    assert_eq!(prod.hosts.len(), 3);

    let state = TuiState::new(fleet, None);
    assert_eq!(state.focus, FocusPanel::Environments);

    let items = state.tree_items();
    assert_eq!(items.len(), 4);
}

#[test]
fn test_tui_key_classification_ignores_release_events() {
    let fleet = rocky::tui::load::Fleet {
        environments: vec![],
        services: vec![],
    };
    let state = TuiState::new(fleet, None);

    let release_event = KeyEvent {
        code: KeyCode::Char('q'),
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Release,
        state: KeyEventState::NONE,
    };
    assert_eq!(classify_key_event(release_event, &state), KeyAction::Ignore);

    let press_event = KeyEvent {
        code: KeyCode::Char('q'),
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert_eq!(classify_key_event(press_event, &state), KeyAction::Quit);
}

#[tokio::test]
async fn test_tui_deploy_checks_mesh_and_scopes_host() {
    let fleet_root = Path::new("examples/supabase");
    let fleet = load_fleet(fleet_root)
        .await
        .expect("failed to load supabase example fleet");

    let mut state = TuiState::new(fleet, None);

    // Explicitly test disconnected mesh preflight
    state.fleet.environments[0].mesh_connected = false;
    let effect = apply_action(&mut state, KeyAction::Deploy);
    assert_eq!(effect, ActionEffect::Continue);
    assert!(matches!(
        state.active_popup,
        ActivePopup::ConfirmJoinMesh { .. }
    ));

    // Now mark mesh connected and test host targeting
    state.active_popup = ActivePopup::None;
    state.fleet.environments[0].mesh_connected = true;

    // Moving down from prod environment selects the first host 'app'
    state.move_down();
    let effect = apply_action(&mut state, KeyAction::Deploy);
    match effect {
        ActionEffect::ExecuteDeploy {
            env_name,
            target_host,
        } => {
            assert_eq!(env_name, "prod");
            assert_eq!(target_host.as_deref(), Some("app"));
        }
        _ => panic!("expected ExecuteDeploy effect, got {effect:?}"),
    }
}

#[test]
fn test_tui_service_modal_replicas_editing() {
    let fleet = rocky::tui::load::Fleet {
        environments: vec![],
        services: vec![],
    };
    let mut state = TuiState::new(fleet, None);

    state.active_popup = ActivePopup::ServiceModal(ServiceForm {
        name: "api".to_string(),
        kind: ServiceKind::Systemd,
        host: None,
        port: "8080".to_string(),
        replicas: 1,
        active_field: 4,
    });

    apply_action(&mut state, KeyAction::InputChar('5'));
    if let ActivePopup::ServiceModal(ref f) = state.active_popup {
        assert_eq!(f.replicas, 5);
    } else {
        panic!("expected ServiceModal");
    }

    apply_action(&mut state, KeyAction::InputChar('2'));
    if let ActivePopup::ServiceModal(ref f) = state.active_popup {
        assert_eq!(f.replicas, 52);
    } else {
        panic!("expected ServiceModal");
    }

    apply_action(&mut state, KeyAction::InputBackspace);
    if let ActivePopup::ServiceModal(ref f) = state.active_popup {
        assert_eq!(f.replicas, 5);
    } else {
        panic!("expected ServiceModal");
    }

    apply_action(&mut state, KeyAction::ToggleFormField);
    if let ActivePopup::ServiceModal(ref f) = state.active_popup {
        assert_eq!(f.replicas, 6);
    } else {
        panic!("expected ServiceModal");
    }
}

#[test]
fn test_tui_secret_viewer_scroll_bounded() {
    let fleet = rocky::tui::load::Fleet {
        environments: vec![],
        services: vec![],
    };
    let mut state = TuiState::new(fleet, None);

    state.active_popup = ActivePopup::SecretViewer(rocky::tui::state::SecretViewerState {
        name: "jwt.age".to_string(),
        environment: "prod".to_string(),
        content: "line 1\nline 2\nline 3\nline 4\nline 5".to_string(),
        scroll: 0,
    });

    apply_action(&mut state, KeyAction::ScrollSecretViewerDown(100));
    if let ActivePopup::SecretViewer(ref v) = state.active_popup {
        assert_eq!(v.scroll, 4);
    } else {
        panic!("expected SecretViewer");
    }

    apply_action(&mut state, KeyAction::ScrollSecretViewerUp(2));
    if let ActivePopup::SecretViewer(ref v) = state.active_popup {
        assert_eq!(v.scroll, 2);
    } else {
        panic!("expected SecretViewer");
    }
}

#[test]
fn test_tui_viewport_scrolling_smooth() {
    let mut scroll = 0;
    let max_rows = 5;

    rocky::tui::state::adjust_scroll(3, &mut scroll, max_rows);
    assert_eq!(scroll, 0);

    rocky::tui::state::adjust_scroll(5, &mut scroll, max_rows);
    assert_eq!(scroll, 1);

    rocky::tui::state::adjust_scroll(4, &mut scroll, max_rows);
    assert_eq!(scroll, 1);

    rocky::tui::state::adjust_scroll(0, &mut scroll, max_rows);
    assert_eq!(scroll, 0);
}
