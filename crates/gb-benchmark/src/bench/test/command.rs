use super::super::command::run_bench_command_with_workspace_for_test;
use super::temp_root;

#[test]
fn command_helper_uses_workspace_context_for_sample() {
    let root = temp_root("command");
    let mut output = Vec::new();
    run_bench_command_with_workspace_for_test(["--sample"], &root, &root, &mut output)
        .expect("sample command should succeed");
    assert!(root.join("test/bench/game.toml").is_file());
}
