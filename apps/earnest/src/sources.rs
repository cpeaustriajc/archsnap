use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const CRATES: &str = "crates";
const SOURCES: &str = "src";
const MANIFEST: &str = "Cargo.toml";
const ALWAYS_READ: [&str; 4] = [
    "Cargo.lock",
    "Cargo.toml",
    "apps/earnest/Cargo.toml",
    "apps/earnest/src",
];

pub(crate) struct SourceFile {
    pub(crate) path: String,
    pub(crate) bytes: Vec<u8>,
}

fn mixed(hash: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(hash, |mixed, byte| {
        (mixed ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

pub(crate) fn fingerprint(files: &[SourceFile]) -> u64 {
    let mut ordered: Vec<&SourceFile> = files.iter().collect();
    ordered.sort_by(|left, right| left.path.cmp(&right.path));
    ordered.iter().fold(FNV_OFFSET, |hash, file| {
        let named = mixed(hash, file.path.as_bytes());
        let sized = mixed(named, &file.bytes.len().to_le_bytes());
        mixed(sized, &file.bytes)
    })
}

pub(crate) fn roots(repository: &Path) -> io::Result<Vec<PathBuf>> {
    let mut roots: Vec<PathBuf> = ALWAYS_READ
        .iter()
        .map(|path| repository.join(path))
        .collect();
    for entry in fs::read_dir(repository.join(CRATES))? {
        let folder = entry?.path();
        if folder.join(MANIFEST).is_file() {
            roots.push(folder.join(MANIFEST));
            roots.push(folder.join(SOURCES));
        }
    }
    roots.sort();
    Ok(roots)
}

fn read_into(path: &Path, repository: &Path, files: &mut Vec<SourceFile>) -> io::Result<()> {
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            read_into(&entry?.path(), repository, files)?;
        }
        return Ok(());
    }
    let relative = path.strip_prefix(repository).unwrap_or(path);
    let parts: Vec<String> = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    files.push(SourceFile {
        path: parts.join("/"),
        bytes: fs::read(path)?,
    });
    Ok(())
}

pub(crate) fn engine(version: &str, repository: &Path) -> io::Result<String> {
    let mut files = Vec::new();
    for root in roots(repository)? {
        read_into(&root, repository, &mut files)?;
    }
    Ok(format!("{version}+{:016x}", fingerprint(&files)))
}

#[cfg(test)]
mod tests {
    use super::{FNV_OFFSET, SourceFile, engine, fingerprint};
    use std::fs;

    fn file(path: &str, text: &str) -> SourceFile {
        SourceFile {
            path: path.to_owned(),
            bytes: text.as_bytes().to_vec(),
        }
    }

    #[test]
    fn no_file_has_the_starting_fingerprint() {
        assert_eq!(fingerprint(&[]), FNV_OFFSET, "nothing was mixed in");
    }

    #[test]
    fn any_change_to_the_sources_changes_the_fingerprint() {
        let before = fingerprint(&[file("a.rs", "fn a() {}"), file("b.rs", "fn b() {}")]);
        let changes = [
            (
                "one byte of one file",
                vec![file("a.rs", "fn a() {}"), file("b.rs", "fn c() {}")],
            ),
            (
                "the name of a file",
                vec![file("a.rs", "fn a() {}"), file("c.rs", "fn b() {}")],
            ),
            ("a file that is gone", vec![file("a.rs", "fn a() {}")]),
            (
                "text that moved from one file to the next",
                vec![file("a.rs", "fn a() {}b"), file(".rs", "fn b() {}")],
            ),
        ];
        for (what, after) in changes {
            assert_ne!(fingerprint(&after), before, "a change of {what}");
        }
    }

    #[test]
    fn the_order_the_files_are_found_in_does_not_change_the_fingerprint() {
        assert_eq!(
            fingerprint(&[file("b.rs", "two"), file("a.rs", "one")]),
            fingerprint(&[file("a.rs", "one"), file("b.rs", "two")]),
            "a folder lists its files in any order"
        );
    }

    #[test]
    fn the_engine_changes_when_a_source_of_a_crate_does_and_not_when_a_test_does() {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path();
        let written = [
            ("Cargo.lock", "lock"),
            ("Cargo.toml", "workspace"),
            ("apps/earnest/Cargo.toml", "app"),
            ("apps/earnest/src/main.rs", "fn main() {}"),
            ("crates/one/Cargo.toml", "one"),
            ("crates/one/src/lib.rs", "pub fn one() {}"),
            ("crates/one/tests/one.rs", "#[test] fn one() {}"),
        ];
        for (path, text) in written {
            fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
            fs::write(root.join(path), text).unwrap();
        }
        let before = engine("1.2.3", root).unwrap();
        assert!(before.starts_with("1.2.3+"), "the version comes first");
        fs::write(root.join("crates/one/tests/one.rs"), "#[test] fn two() {}").unwrap();
        assert_eq!(
            engine("1.2.3", root).unwrap(),
            before,
            "a test is not part of the checker"
        );
        fs::write(
            root.join("crates/one/src/lib.rs"),
            "pub fn one() -> u8 { 1 }",
        )
        .unwrap();
        let after = engine("1.2.3", root).unwrap();
        assert_ne!(after, before, "the code of a crate changed");
        fs::write(root.join("Cargo.lock"), "another lock").unwrap();
        assert_ne!(
            engine("1.2.3", root).unwrap(),
            after,
            "a dependency changed"
        );
    }
}
