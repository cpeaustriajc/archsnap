use std::fs;
use std::path::{Path, PathBuf};

use crate::diag::{Fatal, Warning};
use crate::scan::{Snapshot, SCHEMA, TOOL_VERSION};

pub struct Store {
    pub out: PathBuf,
}

impl Store {
    pub fn open(out: &Path) -> Result<Store, Fatal> {
        let fail = |source| Fatal::OutputNotWritable { path: out.to_path_buf(), source };
        fs::create_dir_all(out.join("snapshots")).map_err(fail)?;
        let probe = out.join(".write-test");
        fs::write(&probe, b"ok").map_err(fail)?;
        let _ = fs::remove_file(probe);
        Ok(Store { out: out.to_path_buf() })
    }

    fn path(&self, week: &str) -> PathBuf {
        self.out.join("snapshots").join(format!("{week}.json"))
    }

    pub fn load(&self, week: &str, sha: &str, depth: usize) -> Result<Option<Snapshot>, Box<Warning>> {
        let p = self.path(week);
        let Ok(text) = fs::read_to_string(&p) else { return Ok(None) };
        match serde_json::from_str::<Snapshot>(&text) {
            Ok(s) if s.schema == SCHEMA && s.tool == TOOL_VERSION && s.sha == sha && s.depth == depth => Ok(Some(s)),
            Ok(_) => Ok(None),
            Err(e) => Err(Box::new(Warning::new(
                "snapshot_corrupt",
                format!("cached snapshot {} is unreadable ({e})", p.display()),
                "it will be rebuilt from git history",
            ))),
        }
    }

    pub fn save(&self, snap: &Snapshot) -> Result<(), Fatal> {
        let p = self.path(&snap.week);
        let json = serde_json::to_string_pretty(snap).expect("snapshot serializes");
        fs::write(&p, json).map_err(|source| Fatal::OutputNotWritable { path: p, source })
    }

    pub fn write_report(&self, html: &str) -> Result<PathBuf, Fatal> {
        let p = self.out.join("index.html");
        fs::write(&p, html).map_err(|source| Fatal::OutputNotWritable { path: p.clone(), source })?;
        Ok(p)
    }
}
