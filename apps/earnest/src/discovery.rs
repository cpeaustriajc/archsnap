use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

const SKIPPED_FOLDERS: [&str; 4] = ["node_modules", "target", "dist", "build"];
const TYPESCRIPT_EXTENSIONS: [&str; 3] = ["ts", "mts", "tsx"];
const TYPESCRIPT_IMPORTS: [&str; 2] = ["\"@earnest/model\"", "'@earnest/model'"];
const RUST_EXTENSION: &str = "rs";
const RUST_MARK: &str = "earnest::model";
const HERE: &str = ".";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Language {
    TypeScript,
    Rust,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Found {
    pub(crate) path: String,
    pub(crate) language: Language,
    pub(crate) source: String,
}

pub(crate) fn language_of(path: &Path) -> Option<Language> {
    let extension = path.extension().and_then(OsStr::to_str)?;
    if TYPESCRIPT_EXTENSIONS.contains(&extension) {
        Some(Language::TypeScript)
    } else if extension == RUST_EXTENSION {
        Some(Language::Rust)
    } else {
        None
    }
}

fn is_model_source(language: Language, source: &str) -> bool {
    match language {
        Language::TypeScript => TYPESCRIPT_IMPORTS
            .iter()
            .any(|import| source.contains(import)),
        Language::Rust => source.contains(RUST_MARK),
    }
}

fn is_skipped_folder(name: &OsStr) -> bool {
    name.to_str()
        .is_none_or(|name| name.starts_with('.') || SKIPPED_FOLDERS.contains(&name))
}

fn model_file(path: &Path) -> Option<Found> {
    let language = language_of(path)?;
    let source = fs::read_to_string(path).ok()?;
    is_model_source(language, &source).then(|| Found {
        path: path.to_string_lossy().into_owned(),
        language,
        source,
    })
}

fn walk(folder: &Path, shown: &Path, found: &mut BTreeMap<PathBuf, Found>) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let shown_entry = shown.join(&name);
        if kind.is_dir() {
            if !is_skipped_folder(&name) {
                walk(&entry.path(), &shown_entry, found);
            }
        } else if kind.is_file() || entry.path().is_file() {
            found.extend(model_file(&shown_entry).map(|file| (shown_entry, file)));
        }
    }
}

pub(crate) fn discover(roots: &[PathBuf]) -> Result<Vec<Found>, String> {
    let here = [PathBuf::from(HERE)];
    let roots = if roots.is_empty() { &here } else { roots };
    let mut found = BTreeMap::new();
    for root in roots {
        if root.is_dir() {
            let shown = if root == Path::new(HERE) {
                Path::new("")
            } else {
                root.as_path()
            };
            walk(root, shown, &mut found);
        } else if root.is_file() {
            found.extend(model_file(root).map(|file| (root.clone(), file)));
        } else {
            return Err(format!(
                "`{}` is not a file or a folder",
                root.to_string_lossy()
            ));
        }
    }
    Ok(found.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::{Language, discover, language_of};
    use std::fs;
    use std::path::{Path, PathBuf};

    const TYPESCRIPT: &str = "import { m } from \"@earnest/model\";\n";
    const RUST: &str = "use earnest::model::Model;\n";

    fn folder_with(files: &[(&str, &str)]) -> tempfile::TempDir {
        let folder = tempfile::tempdir().expect("a temporary folder can be made");
        for (relative, text) in files {
            let target = folder.path().join(relative);
            fs::create_dir_all(target.parent().expect("a file has a folder"))
                .expect("a folder can be made");
            fs::write(target, text).expect("a file can be written");
        }
        folder
    }

    fn names_under(folder: &Path, roots: &[PathBuf]) -> Vec<String> {
        discover(roots)
            .expect("every root exists")
            .into_iter()
            .map(|found| {
                Path::new(&found.path)
                    .strip_prefix(folder)
                    .expect("a found file is under its root")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    #[test]
    fn the_extension_picks_the_language() {
        for name in ["a.ts", "a.mts", "a.tsx", "deep/a.model.ts"] {
            assert_eq!(
                language_of(Path::new(name)),
                Some(Language::TypeScript),
                "{name} is TypeScript"
            );
        }
        assert_eq!(
            language_of(Path::new("a.rs")),
            Some(Language::Rust),
            "a.rs is Rust"
        );
        for name in ["a.js", "a.cts", "a.d", "ts", "a"] {
            assert_eq!(language_of(Path::new(name)), None, "{name} is no model");
        }
    }

    #[test]
    fn only_files_that_use_the_library_are_models() {
        let folder = folder_with(&[
            ("a.model.ts", TYPESCRIPT),
            ("single.mts", "import { m } from '@earnest/model';\n"),
            ("plain.ts", "export const one = 1;\n"),
            ("other.ts", "import { m } from \"@earnest/models\";\n"),
            ("gate.rs", RUST),
            ("plain.rs", "pub fn one() -> u8 { 1 }\n"),
            ("notes.md", TYPESCRIPT),
        ]);
        assert_eq!(
            names_under(folder.path(), &[folder.path().to_path_buf()]),
            ["a.model.ts", "gate.rs", "single.mts"],
            "the files that import the library, in path order"
        );
    }

    #[test]
    fn build_folders_and_hidden_folders_are_skipped() {
        let folder = folder_with(&[
            ("src/a.ts", TYPESCRIPT),
            ("node_modules/pkg/a.ts", TYPESCRIPT),
            ("target/a.rs", RUST),
            ("dist/a.ts", TYPESCRIPT),
            ("build/a.ts", TYPESCRIPT),
            (".git/a.ts", TYPESCRIPT),
            ("src/.cache/a.ts", TYPESCRIPT),
            ("src/builder/a.ts", TYPESCRIPT),
        ]);
        assert_eq!(
            names_under(folder.path(), &[folder.path().to_path_buf()]),
            ["src/a.ts", "src/builder/a.ts"],
            "only the files outside the skipped folders"
        );
    }

    #[test]
    fn files_come_in_path_order_whatever_order_the_roots_have_and_each_once() {
        let folder = folder_with(&[
            ("b/one.ts", TYPESCRIPT),
            ("a/two.ts", TYPESCRIPT),
            ("a/deep/three.ts", TYPESCRIPT),
            ("a.ts", TYPESCRIPT),
        ]);
        let root = folder.path();
        let roots = [
            root.join("b"),
            root.join("a"),
            root.join("a/two.ts"),
            root.join("a.ts"),
        ];
        assert_eq!(
            names_under(root, &roots),
            ["a/deep/three.ts", "a/two.ts", "a.ts", "b/one.ts"],
            "a folder's files come before the next file beside it"
        );
    }

    #[test]
    fn a_root_that_does_not_exist_is_an_error() {
        let folder = folder_with(&[]);
        let missing = folder.path().join("nowhere");
        let error = discover(&[missing]).expect_err("the root is missing");
        assert!(
            error.contains("nowhere"),
            "the error names the root: {error}"
        );
    }
}
