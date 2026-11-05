//! The wasm sandbox: package names, one folder per student, the size cap, and how a child ends.

use std::path::{Path, PathBuf};
use std::time::Duration;

use uuid::Uuid;

use crate::core::config::{SandboxSettings, WASM_DESCRIPTION, WASM_RUNTIME};
use crate::core::tests::support::ctx;
use crate::core::traits::tools::sandbox::Sandbox;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::tools::sandbox::{SandboxError, SandboxRequest};
use crate::runtime::tools::sandbox::Wording;
use crate::runtime::tools::sandbox::wasm::runner::package_id;
use crate::runtime::tools::sandbox::wasm::{Folders, WasmLimits, WasmSandbox};

/// A fresh folder under the system temp folder.
fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sparky-wasm-{}", Uuid::new_v4()));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn limits(root: &Path, mb: u32) -> WasmLimits {
    WasmLimits {
        folders: Folders {
            packages: root.join("packages"),
            cache: root.join("cache"),
            work: root.join("work"),
        },
        timeout: Duration::from_secs(2),
        max_output_chars: 4_000,
        max_running: 2,
        workspace_mb: mb,
        recent_commands: 10,
    }
}

/// A shell script that stands in for the engine child.
fn child(root: &Path, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = root.join("child.sh");
    let _ = std::fs::write(&path, format!("#!/bin/sh\n{body}\n"));
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
    path
}

fn run(command: &str) -> SandboxRequest {
    SandboxRequest {
        command: command.into(),
        session: None,
    }
}

#[test]
fn a_package_file_names_its_registry_name_and_version() {
    let id = package_id(Path::new("/p/wasmer__python@3.13.15.webc"));
    assert_eq!(
        id.map(|(n, v)| (n, v.to_string())),
        Some(("wasmer/python".into(), "3.13.15".into()))
    );
    assert!(package_id(Path::new("/p/python.webc")).is_none());
    assert!(package_id(Path::new("/p/wasmer__bash@x.webc")).is_none());
    assert!(package_id(Path::new("/p/wasmer__bash@1.0.0.tar")).is_none());
}

#[test]
fn each_student_has_a_folder_of_their_own() {
    let a = WasmSandbox::folder_name(&ctx());
    let b = WasmSandbox::folder_name(&RequestContext::new("g", "other", Duration::from_secs(5)));
    let c = WasmSandbox::folder_name(&RequestContext::new("h", "u", Duration::from_secs(5)));
    assert_eq!(a, WasmSandbox::folder_name(&ctx()));
    assert_ne!(a, b);
    assert_ne!(a, c);
}

#[tokio::test]
async fn a_file_put_lands_in_the_student_folder_under_work() {
    let root = scratch();
    let sandbox = WasmSandbox::new(limits(&root, 1), child(&root, "exit 0"));
    let Ok(path) = sandbox.put(&ctx(), "s1", "notes.txt", b"hello").await else {
        unreachable!("the put lands")
    };
    assert_eq!(path, "/work/notes.txt");
    let on_disk = root
        .join("work")
        .join(WasmSandbox::folder_name(&ctx()))
        .join("notes.txt");
    assert_eq!(std::fs::read(on_disk).ok(), Some(b"hello".to_vec()));
    let bad = sandbox.put(&ctx(), "s1", "../x", b"no").await;
    assert!(matches!(bad, Err(SandboxError::Refused(_))));
}

#[tokio::test]
async fn the_child_exit_status_and_output_come_back() {
    let root = scratch();
    let sandbox = WasmSandbox::new(limits(&root, 1), child(&root, "cat; echo oops >&2; exit 3"));
    let Ok(out) = sandbox.run(&ctx(), &run("echo hi")).await else {
        unreachable!("the child runs")
    };
    assert_eq!(out.exit_code, 3);
    assert_eq!(out.stdout, "echo hi");
    assert_eq!(out.stderr.trim(), "oops");
    assert_eq!(
        sandbox.commands().first().and_then(|c| c.exit_code),
        Some(3)
    );
    assert_eq!(sandbox.sessions().len(), 1);
}

#[tokio::test]
async fn a_child_that_cannot_set_up_is_a_runtime_error() {
    let root = scratch();
    let sandbox = WasmSandbox::new(
        limits(&root, 1),
        child(&root, "echo no packages >&2; exit 125"),
    );
    let ran = sandbox.run(&ctx(), &run("true")).await;
    assert!(matches!(ran, Err(SandboxError::Runtime(m)) if m.contains("no packages")));
}

#[tokio::test]
async fn a_command_past_its_budget_is_killed() {
    let root = scratch();
    let sandbox = WasmSandbox::new(limits(&root, 1), child(&root, "sleep 30"));
    let ran = sandbox.run(&ctx(), &run("loop")).await;
    assert!(matches!(ran, Err(SandboxError::Timeout)));
    assert_eq!(sandbox.commands().first().and_then(|c| c.exit_code), None);
}

#[tokio::test]
async fn a_full_folder_is_refused_before_anything_runs() {
    let root = scratch();
    let sandbox = WasmSandbox::new(limits(&root, 0), child(&root, "exit 0"));
    let folder = root.join("work").join(WasmSandbox::folder_name(&ctx()));
    let _ = std::fs::create_dir_all(&folder);
    let _ = std::fs::write(folder.join("big"), b"x");
    let ran = sandbox.run(&ctx(), &run("true")).await;
    assert!(matches!(ran, Err(SandboxError::Refused(m)) if m.contains("delete files")));
    assert_eq!(sandbox.commands().len(), 0);
}

#[tokio::test]
async fn killing_a_workspace_deletes_its_folder() {
    let root = scratch();
    let sandbox = WasmSandbox::new(limits(&root, 1), child(&root, "exit 0"));
    let _ = sandbox.run(&ctx(), &run("true")).await;
    let name = WasmSandbox::folder_name(&ctx());
    assert!(sandbox.kill(&name).await.is_ok());
    assert!(!root.join("work").join(&name).exists());
    assert!(matches!(
        sandbox.kill(&name).await,
        Err(SandboxError::Refused(_))
    ));
}

#[tokio::test]
async fn no_shell_package_fails_the_probe() {
    let root = scratch();
    let _ = std::fs::create_dir_all(root.join("packages"));
    let _ = std::fs::write(
        root.join("packages").join("wasmer__python@3.13.15.webc"),
        b"",
    );
    let sandbox = WasmSandbox::new(limits(&root, 1), child(&root, "exit 0"));
    let probed = sandbox.probe().await;
    assert!(matches!(probed, Err(SandboxError::Runtime(m)) if m.contains("wasmer/bash")));
}

#[test]
fn attached_files_are_read_in_the_engine_and_the_wording_says_wasm() {
    let root = scratch();
    let sandbox = WasmSandbox::new(limits(&root, 1), child(&root, "exit 0"));
    assert!(!sandbox.converts());
    let cfg = SandboxSettings {
        runtime: WASM_RUNTIME.into(),
        ..SandboxSettings::default()
    };
    assert_eq!(Wording::from(&cfg).description, WASM_DESCRIPTION);
}
