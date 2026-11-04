//! Backup copies: a consistent file per run, newest kept.

use std::time::Duration;

use uuid::Uuid;

use crate::stores::sqlite::{BackupPlan, SqliteStore, StoreOptions, backup_once};

#[tokio::test]
async fn a_backup_writes_a_copy_and_keeps_only_the_newest() {
    let root = std::env::temp_dir().join(format!("sparky-store-{}", Uuid::new_v4()));
    let options = StoreOptions {
        path: root.join("sparky.db").to_string_lossy().into_owned(),
        max_connections: 2,
        busy_timeout: Duration::from_secs(1),
    };
    let Ok(store) = SqliteStore::open(&options).await else {
        unreachable!("the file opens")
    };
    let dir = root.join("backups");
    assert!(std::fs::create_dir_all(&dir).is_ok());
    for old in ["sparky-20200101T000000Z.db", "sparky-20200102T000000Z.db"] {
        assert!(std::fs::write(dir.join(old), b"").is_ok());
    }
    let plan = BackupPlan {
        dir: dir.clone(),
        every: Duration::from_secs(3600),
        kept: 2,
    };
    let Ok(copy) = backup_once(store.pool(), &plan).await else {
        unreachable!("the backup runs")
    };
    assert!(copy.exists());
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map(|d| {
            d.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    assert_eq!(names.len(), 2);
    assert_eq!(names[0], "sparky-20200102T000000Z.db");

    let restored = StoreOptions {
        path: copy.to_string_lossy().into_owned(),
        ..options
    };
    assert!(SqliteStore::open(&restored).await.is_ok());
    let _ = std::fs::remove_dir_all(root);
}
