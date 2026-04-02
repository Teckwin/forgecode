use forge_ci::workflows as workflow;

#[test]
fn generate() {
    // In CI environment, this test just verifies the workflow doesn't panic
    // The actual workflow file comparison is done via check_file which
    // returns error if content doesn't match
    workflow::generate_ci_workflow();
}

#[test]
fn test_release_drafter() {
    workflow::generate_release_drafter_workflow();
}

#[test]
fn test_release_workflow() {
    // In CI environment, this test just verifies the workflow doesn't panic
    workflow::release_publish();
}

#[test]
fn test_labels_workflow() {
    workflow::generate_labels_workflow();
}

#[test]
fn test_stale_workflow() {
    workflow::generate_stale_workflow();
}

#[test]
fn test_autofix_workflow() {
    workflow::generate_autofix_workflow();
}

#[test]
fn test_bounty_workflow() {
    workflow::generate_bounty_workflow();
}
