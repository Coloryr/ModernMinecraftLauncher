use super::*;

#[test]
fn offline_launch_requires_a_premium_login() {
    assert!(offline_needs_premium(AuthType::Offline, false));
    assert!(!offline_needs_premium(AuthType::Offline, true));
    assert!(!offline_needs_premium(AuthType::OAuth, false));
    assert!(!offline_needs_premium(AuthType::Nide8, false));
}

