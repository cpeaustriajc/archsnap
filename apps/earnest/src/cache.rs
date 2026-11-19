use crate::run::{Bounds, Checked};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

const FOLDER: &str = ".earnest";
const ENTRIES: &str = "cache";
const IGNORE_FILE: &str = ".gitignore";
const IGNORE_EVERYTHING: &str = "*\n";
const ENGINE: &str = env!("CARGO_PKG_VERSION");
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

static WRITES: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Key {
    engine: String,
    file: String,
    source: String,
    max_states: usize,
    time_limit_seconds: Option<u64>,
    rule: Option<String>,
}

impl Key {
    pub(crate) fn new(file: &str, source: &str, bounds: Bounds, rule: Option<&str>) -> Self {
        Self {
            engine: ENGINE.to_owned(),
            file: file.to_owned(),
            source: source.to_owned(),
            max_states: bounds.max_states,
            time_limit_seconds: bounds.time_limit.map(|limit| limit.as_secs()),
            rule: rule.map(str::to_owned),
        }
    }

    pub(crate) fn has_time_limit(&self) -> bool {
        self.time_limit_seconds.is_some()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Entry {
    key: Key,
    models: Vec<Checked>,
}

#[derive(Debug)]
pub(crate) struct Cache {
    root: PathBuf,
}

fn fingerprint(text: &str) -> u64 {
    text.bytes().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
    })
}

impl Cache {
    pub(crate) fn under(folder: &Path) -> Self {
        Self {
            root: folder.join(FOLDER),
        }
    }

    fn entry_path(&self, file: &str) -> PathBuf {
        self.root
            .join(ENTRIES)
            .join(format!("{:016x}.json", fingerprint(file)))
    }

    pub(crate) fn load(&self, key: &Key) -> Vec<Checked> {
        let Ok(text) = fs::read_to_string(self.entry_path(&key.file)) else {
            return Vec::new();
        };
        match serde_json::from_str::<Entry>(&text) {
            Ok(entry) if entry.key == *key => entry.models,
            Ok(_) | Err(_) => Vec::new(),
        }
    }

    pub(crate) fn store(&self, key: &Key, models: &[Checked]) -> io::Result<()> {
        let target = self.entry_path(&key.file);
        let folder = self.root.join(ENTRIES);
        fs::create_dir_all(&folder)?;
        let ignore = self.root.join(IGNORE_FILE);
        if !ignore.exists() {
            fs::write(ignore, IGNORE_EVERYTHING)?;
        }
        let entry = Entry {
            key: key.clone(),
            models: models.to_vec(),
        };
        let text = serde_json::to_string(&entry).map_err(io::Error::other)?;
        let unfinished = folder.join(format!(
            ".{:016x}.{}.{}.tmp",
            fingerprint(&key.file),
            process::id(),
            WRITES.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&unfinished, text)?;
        fs::rename(&unfinished, &target).inspect_err(|_| {
            fs::remove_file(&unfinished).unwrap_or_default();
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Cache, Key, fingerprint};
    use crate::run::{Bounds, Checked, ModelResult, Status};
    use std::fs;
    use std::time::Duration;

    const FILE: &str = "src/counter.model.ts";
    const SOURCE: &str = "export const Counter = 1;\n";

    fn bounds() -> Bounds {
        Bounds {
            max_states: 5_000_000,
            time_limit: None,
            jobs: 4,
        }
    }

    fn key() -> Key {
        Key::new(FILE, SOURCE, bounds(), None)
    }

    fn checked(name: &str, states: usize) -> Checked {
        Checked {
            result: ModelResult {
                name: name.to_owned(),
                file: FILE.to_owned(),
                line: 3,
                status: Status::Pass,
                states,
                milliseconds: 7,
                cached: false,
                reason: None,
                rules: Vec::new(),
                findings: Vec::new(),
            },
            detail: vec!["broken  rule".to_owned()],
        }
    }

    #[test]
    fn what_was_stored_is_loaded_under_the_same_key() {
        let folder = tempfile::tempdir().unwrap();
        let cache = Cache::under(folder.path());
        assert!(
            cache.load(&key()).is_empty(),
            "nothing before the first run"
        );
        let models = [checked("Counter", 4), checked("Other", 9)];
        cache.store(&key(), &models).unwrap();
        assert_eq!(cache.load(&key()), models, "the stored results");
    }

    #[test]
    fn anything_that_can_change_the_result_misses() {
        let folder = tempfile::tempdir().unwrap();
        let cache = Cache::under(folder.path());
        cache.store(&key(), &[checked("Counter", 4)]).unwrap();
        let fewer_states = Bounds {
            max_states: 10,
            ..bounds()
        };
        let timed = Bounds {
            time_limit: Some(Duration::from_secs(1)),
            ..bounds()
        };
        let misses = [
            ("the source", Key::new(FILE, "changed", bounds(), None)),
            (
                "the state limit",
                Key::new(FILE, SOURCE, fewer_states, None),
            ),
            ("the time limit", Key::new(FILE, SOURCE, timed, None)),
            (
                "the rule",
                Key::new(FILE, SOURCE, bounds(), Some("bounded")),
            ),
            (
                "the engine version",
                Key {
                    engine: "0.0.0-other".to_owned(),
                    ..key()
                },
            ),
        ];
        for (what, changed) in misses {
            assert!(
                cache.load(&changed).is_empty(),
                "a change of {what} checks the model again"
            );
        }
    }

    #[test]
    fn the_number_of_workers_does_not_change_the_key() {
        let one = Bounds {
            jobs: 1,
            ..bounds()
        };
        assert_eq!(
            Key::new(FILE, SOURCE, one, None),
            key(),
            "the result is the same for every number of workers"
        );
    }

    #[test]
    fn another_file_has_its_own_entry() {
        let folder = tempfile::tempdir().unwrap();
        let cache = Cache::under(folder.path());
        cache.store(&key(), &[checked("Counter", 4)]).unwrap();
        let other = Key::new("src/other.model.ts", SOURCE, bounds(), None);
        assert!(
            cache.load(&other).is_empty(),
            "the other file was never stored"
        );
        cache.store(&other, &[checked("Other", 2)]).unwrap();
        assert_eq!(
            cache.load(&key()),
            [checked("Counter", 4)],
            "the first file"
        );
        assert_eq!(cache.load(&other), [checked("Other", 2)], "the second file");
    }

    #[test]
    fn a_damaged_entry_is_a_miss_and_is_replaced() {
        let folder = tempfile::tempdir().unwrap();
        let cache = Cache::under(folder.path());
        cache.store(&key(), &[checked("Counter", 4)]).unwrap();
        fs::write(cache.entry_path(FILE), "{ not json").unwrap();
        assert!(cache.load(&key()).is_empty(), "a damaged entry is not used");
        cache.store(&key(), &[checked("Counter", 4)]).unwrap();
        assert_eq!(
            cache.load(&key()),
            [checked("Counter", 4)],
            "it is replaced"
        );
    }

    #[test]
    fn everything_is_written_under_dot_earnest_and_no_unfinished_file_stays() {
        let folder = tempfile::tempdir().unwrap();
        let cache = Cache::under(folder.path());
        cache.store(&key(), &[checked("Counter", 4)]).unwrap();
        let top: Vec<String> = fs::read_dir(folder.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(top, [".earnest"], "one folder in the folder of the run");
        assert_eq!(
            fs::read_to_string(folder.path().join(".earnest/.gitignore")).unwrap(),
            "*\n",
            "git ignores the cache"
        );
        let entries: Vec<String> = fs::read_dir(folder.path().join(".earnest/cache"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            entries,
            [format!("{:016x}.json", fingerprint(FILE))],
            "one finished entry"
        );
    }

    #[test]
    fn many_writers_at_once_leave_one_whole_entry() {
        let folder = tempfile::tempdir().unwrap();
        let cache = Cache::under(folder.path());
        std::thread::scope(|scope| {
            for writer in 0..8 {
                let cache = &cache;
                scope.spawn(move || {
                    for round in 0..20 {
                        cache
                            .store(&key(), &[checked("Counter", writer * 100 + round)])
                            .unwrap();
                        let loaded = cache.load(&key());
                        assert_eq!(loaded.len(), 1, "a reader sees a whole entry");
                    }
                });
            }
        });
        let entries = fs::read_dir(folder.path().join(".earnest/cache"))
            .unwrap()
            .count();
        assert_eq!(entries, 1, "no unfinished file stays behind");
    }

    #[test]
    fn the_fingerprint_is_the_fnv_hash_of_the_path() {
        assert_eq!(fingerprint(""), 0xcbf2_9ce4_8422_2325, "the empty text");
        assert_eq!(fingerprint("a"), 0xaf63_dc4c_8601_ec8c, "one letter");
        assert_ne!(fingerprint("a/b.ts"), fingerprint("a/c.ts"), "two paths");
    }
}
