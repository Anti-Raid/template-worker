use std::{io::Write, process::{Command, Stdio}};

use crate::migrations::Migration;

const SEAWEED_TO_BLOB_PY: &'static str = include_str!("migrate_backups.py");

pub static MIGRATION: Migration = Migration {
    id: "seaweed_to_blob",
    description: "Convert seaweedfs s3 -> blob api",
    up: |_pool| {
        Box::pin(async move {
            let mut child = Command::new("python3")
            .arg("-")
            .stdin(Stdio::piped())
            .spawn()?;

            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(SEAWEED_TO_BLOB_PY.as_bytes())?;
            } 
            let rc = child.wait()?;

            if !rc.success() {
                if std::env::var("IGNORE_SEAWEED_ERRORS").unwrap_or_default() == "1" {
                    Ok(())
                } else {
                    Err("Failed to execute seaweed migration script".into())
                }
            } else {
                Ok(())
            }
        })
    },
};
