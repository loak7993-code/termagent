mod common;

use std::path::Path;

use serde_json::json;
use termagent_core::tools::{self, ToolCtx};

fn ctx(root: &Path) -> ToolCtx {
    ToolCtx::new(root.to_path_buf())
}

async fn run(root: &Path, name: &str, args: serde_json::Value) -> termagent_core::Result<String> {
    let all = tools::all();
    let tool = all.iter().find(|t| t.name() == name).unwrap();
    let c = ctx(root);
    tool.run(&args, &c).await
}

#[test]
fn every_tool_has_a_schema() {
    let all = tools::all();
    let schemas = tools::schemas(&all);
    assert_eq!(schemas.as_array().unwrap().len(), 9);
    for s in schemas.as_array().unwrap() {
        let f = &s["function"];
        assert!(!f["name"].as_str().unwrap().is_empty());
        assert!(!f["description"].as_str().unwrap().is_empty());
        assert_eq!(f["parameters"]["type"], "object");
    }
}

#[tokio::test]
async fn read_numbers_lines_and_caps_output() {
    let dir = tempfile::tempdir().unwrap();
    let body: String = (1..=2500).map(|i| format!("line {i}\n")).collect();
    std::fs::write(dir.path().join("big.txt"), body).unwrap();

    let out = run(dir.path(), "read", json!({"path": "big.txt"}))
        .await
        .unwrap();
    assert!(out.starts_with("     1\tline 1"));
    assert!(out.contains("… 500 more lines"));

    let slice = run(
        dir.path(),
        "read",
        json!({"path": "big.txt", "offset": 10, "limit": 2}),
    )
    .await
    .unwrap();
    assert_eq!(slice.lines().count(), 3);
    assert!(slice.starts_with("    10\tline 10"));
}

#[tokio::test]
async fn read_refuses_binary_and_escapes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("bin"), [0u8, 1, 2, 0, 3]).unwrap();
    assert!(run(dir.path(), "read", json!({"path": "bin"}))
        .await
        .is_err());
    assert!(run(dir.path(), "read", json!({"path": "../../etc/passwd"}))
        .await
        .is_err());
    assert!(run(dir.path(), "read", json!({"path": "/etc/passwd"}))
        .await
        .is_err());
}

#[tokio::test]
async fn write_creates_parents_and_blocks_escapes() {
    let dir = tempfile::tempdir().unwrap();
    let out = run(
        dir.path(),
        "write",
        json!({"path": "src/deep/file.txt", "content": "hello"}),
    )
    .await
    .unwrap();
    assert!(out.contains("5 bytes"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("src/deep/file.txt")).unwrap(),
        "hello"
    );
    assert!(run(
        dir.path(),
        "write",
        json!({"path": "../outside.txt", "content": "x"})
    )
    .await
    .is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn write_blocks_symlink_escape() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();
    assert!(run(
        dir.path(),
        "write",
        json!({"path": "link/x.txt", "content": "x"})
    )
    .await
    .is_err());
}

#[tokio::test]
async fn edit_requires_unique_match() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f.txt");
    std::fs::write(&path, "alpha\nbeta\nalpha\n").unwrap();
    assert!(run(
        dir.path(),
        "edit",
        json!({"path": "f.txt", "old_string": "alpha", "new_string": "gamma"})
    )
    .await
    .is_err());
    let out = run(
        dir.path(),
        "edit",
        json!({"path": "f.txt", "old_string": "alpha", "new_string": "gamma", "replace_all": true}),
    )
    .await
    .unwrap();
    assert!(out.contains("2 replacements"));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "gamma\nbeta\ngamma\n"
    );
    assert!(run(
        dir.path(),
        "edit",
        json!({"path": "f.txt", "old_string": "zzz", "new_string": "y"})
    )
    .await
    .is_err());
}

#[tokio::test]
async fn ls_glob_grep_find_fixtures() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("src/main.rs"),
        "fn main() { println!(\"Hello\"); }\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("README.md"), "hello world\n").unwrap();

    let ls = run(dir.path(), "ls", json!({})).await.unwrap();
    assert!(ls.contains("src/"));
    assert!(ls.contains("README.md"));

    let glob = run(dir.path(), "glob", json!({"pattern": "**/*.rs"}))
        .await
        .unwrap();
    assert!(glob.contains("src/main.rs"));
    assert!(!glob.contains("README.md"));

    let hits = run(dir.path(), "grep", json!({"pattern": "hello"}))
        .await
        .unwrap();
    assert!(hits.contains("src/main.rs:1:"));
    assert!(hits.contains("README.md:1:"));
}

#[tokio::test]
async fn bash_runs_in_workspace_without_privilege_escalation() {
    let dir = tempfile::tempdir().unwrap();
    let out = run(dir.path(), "bash", json!({"command": "pwd"}))
        .await
        .unwrap();
    assert!(out.contains(dir.path().to_string_lossy().as_ref()));

    let denied = run(dir.path(), "bash", json!({"command": "sudo ls"}))
        .await
        .unwrap_err();
    assert!(denied.to_string().contains("privilege escalation"));

    let code = run(dir.path(), "bash", json!({"command": "exit 3"}))
        .await
        .unwrap();
    assert!(code.contains("[exit 3]"));
}

#[tokio::test]
async fn bash_scrubs_api_keys_from_child_env() {
    std::env::set_var("TERMAGENT_TEST_API_KEY", "super-secret");
    let dir = tempfile::tempdir().unwrap();
    let out = run(
        dir.path(),
        "bash",
        json!({"command": "echo key=${TERMAGENT_TEST_API_KEY:-unset}"}),
    )
    .await
    .unwrap();
    assert!(out.contains("key=unset"), "child saw: {out}");
    std::env::remove_var("TERMAGENT_TEST_API_KEY");
}

#[tokio::test]
async fn bash_times_out_and_truncates() {
    let dir = tempfile::tempdir().unwrap();
    let timed = run(
        dir.path(),
        "bash",
        json!({"command": "sleep 5", "timeout": 1}),
    )
    .await;
    assert!(timed.unwrap_err().to_string().contains("timed out"));

    let big = run(
        dir.path(),
        "bash",
        json!({"command": "yes termagent | head -c 90000"}),
    )
    .await
    .unwrap();
    assert!(big.contains("truncated"));
}

#[tokio::test]
async fn todo_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let c = ctx(dir.path());
    let tool = tools::all()
        .into_iter()
        .find(|t| t.name() == "todo")
        .unwrap();
    let out = tool
        .run(
            &json!({"items": [
                {"content": "write tests", "status": "doing"},
                {"content": "ship", "status": "todo"}
            ]}),
            &c,
        )
        .await
        .unwrap();
    assert!(out.contains("[~] write tests"));
    assert!(out.contains("[ ] ship"));
    let read_back = tool.run(&json!({}), &c).await.unwrap();
    assert!(read_back.contains("write tests"));
}

#[tokio::test]
async fn webfetch_rejects_non_http() {
    let dir = tempfile::tempdir().unwrap();
    let err = run(dir.path(), "webfetch", json!({"url": "file:///etc/passwd"}))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("http"));
}
