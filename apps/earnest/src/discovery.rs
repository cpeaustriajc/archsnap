use earnest_diagnostics::{Place, Problem};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::Utf8Error;

const SKIPPED_FOLDERS: [&str; 4] = ["node_modules", "target", "dist", "build"];
const TYPESCRIPT_EXTENSIONS: [&str; 3] = ["ts", "mts", "tsx"];
const TYPESCRIPT_IMPORTS: [&str; 2] = ["\"@earnest/model\"", "'@earnest/model'"];
const RUST_EXTENSION: &str = "rs";
const RUST_MARKS: [&str; 4] = [
    "earnest::model",
    "earnest::{",
    "useearnestas",
    "externcrateearnest",
];
const HERE: &str = ".";
pub(crate) const IN_EVERY_MODEL_FILE: &str = "earnest";
const UTF16_LITTLE_ENDIAN: [u8; 2] = [0xFF, 0xFE];
const UTF16_BIG_ENDIAN: [u8; 2] = [0xFE, 0xFF];
const NOT_UTF8: &str = "the file is not UTF-8 text, so the checker can't read it: save it as UTF-8";
const TOP_OF_FILE: Place = Place { line: 1, column: 1 };

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Language {
    TypeScript,
    Rust,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Text {
    Model { language: Language, source: String },
    Unread(Problem),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Found {
    pub(crate) path: String,
    pub(crate) text: Text,
}

impl Found {
    pub(crate) fn model(path: &str, language: Language, source: &str) -> Self {
        Self {
            path: path.to_owned(),
            text: Text::Model {
                language,
                source: source.to_owned(),
            },
        }
    }

    fn unread(path: &Path, problem: Problem) -> Self {
        Self {
            path: path.to_string_lossy().into_owned(),
            text: Text::Unread(problem),
        }
    }
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

pub(crate) fn is_model_source(language: Language, source: &str) -> bool {
    match language {
        Language::TypeScript => TYPESCRIPT_IMPORTS
            .iter()
            .any(|import| source.contains(import)),
        Language::Rust => {
            let unspaced: String = source.split_whitespace().collect();
            RUST_MARKS.iter().any(|mark| unspaced.contains(mark))
        }
    }
}

fn is_skipped_folder(name: &OsStr) -> bool {
    name.to_str()
        .is_none_or(|name| name.starts_with('.') || SKIPPED_FOLDERS.contains(&name))
}

pub(crate) fn is_searched(below: &Path) -> bool {
    let folders_are_searched = below.parent().is_none_or(|folders| {
        folders
            .components()
            .all(|folder| !is_skipped_folder(folder.as_os_str()))
    });
    folders_are_searched && language_of(below).is_some()
}

fn in_backticks(path: &Path) -> String {
    let name = path.file_name().unwrap_or(path.as_os_str());
    format!("`{}`", name.to_string_lossy())
}

fn cant_be_opened(path: &Path, what: &str, error: &io::Error) -> Problem {
    Problem::new(
        &path.to_string_lossy(),
        TOP_OF_FILE,
        &in_backticks(path),
        &format!(
            "the {what} can't be opened, so the checker can't tell whether it holds a model: {error}"
        ),
    )
}

fn place_after(text: &str) -> Place {
    let line = text.matches('\n').count().saturating_add(1);
    let column = text
        .rsplit('\n')
        .next()
        .map_or(0, |last| last.chars().count())
        .saturating_add(1);
    Place {
        line: u32::try_from(line).unwrap_or(u32::MAX),
        column: u32::try_from(column).unwrap_or(u32::MAX),
    }
}

fn utf16_text(bytes: &[u8]) -> Option<String> {
    let (mark, rest) = bytes.split_first_chunk::<2>()?;
    let unit: fn([u8; 2]) -> u16 = match *mark {
        UTF16_LITTLE_ENDIAN => u16::from_le_bytes,
        UTF16_BIG_ENDIAN => u16::from_be_bytes,
        _ => return None,
    };
    let units: Vec<u16> = rest
        .chunks_exact(2)
        .filter_map(|pair| pair.first_chunk::<2>().copied())
        .map(unit)
        .collect();
    Some(String::from_utf16_lossy(&units))
}

fn not_utf8(path: &Path, language: Language, bytes: &[u8], error: Utf8Error) -> Option<Problem> {
    let shown = path.to_string_lossy();
    if let Some(decoded) = utf16_text(bytes) {
        return is_model_source(language, &decoded)
            .then(|| Problem::new(&shown, TOP_OF_FILE, "`UTF-16`", NOT_UTF8));
    }
    let lossy = String::from_utf8_lossy(bytes);
    if !is_model_source(language, &lossy) {
        return None;
    }
    let (before, after) = bytes.split_at_checked(error.valid_up_to())?;
    let place = place_after(&String::from_utf8_lossy(before));
    let byte = after.first().copied().unwrap_or_default();
    Some(Problem::new(
        &shown,
        place,
        &format!("`byte 0x{byte:02X}`"),
        NOT_UTF8,
    ))
}

fn model_file(path: &Path) -> Option<Found> {
    let language = language_of(path)?;
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => return Some(Found::unread(path, cant_be_opened(path, "file", &error))),
    };
    match String::from_utf8(bytes) {
        Ok(source) => is_model_source(language, &source)
            .then(|| Found::model(&path.to_string_lossy(), language, &source)),
        Err(refused) => {
            let error = refused.utf8_error();
            not_utf8(path, language, refused.as_bytes(), error)
                .map(|problem| Found::unread(path, problem))
        }
    }
}

fn walk(folder: &Path, shown: &Path, found: &mut BTreeMap<PathBuf, Found>) {
    let listed: io::Result<Vec<fs::DirEntry>> = fs::read_dir(folder).and_then(Iterator::collect);
    let entries = match listed {
        Ok(entries) => entries,
        Err(error) => {
            let named = if shown.as_os_str().is_empty() {
                folder
            } else {
                shown
            };
            let problem = cant_be_opened(named, "folder", &error);
            found.insert(named.to_path_buf(), Found::unread(named, problem));
            return;
        }
    };
    for entry in entries {
        let name = entry.file_name();
        let shown_entry = shown.join(&name);
        let Ok(kind) = entry.file_type() else {
            found.extend(model_file(&shown_entry).map(|file| (shown_entry, file)));
            continue;
        };
        if kind.is_dir() {
            if !is_skipped_folder(&name) {
                walk(&entry.path(), &shown_entry, found);
            }
        } else if kind.is_file() || (kind.is_symlink() && !entry.path().is_dir()) {
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
    use super::{Found, Language, Text, discover, language_of, walk};
    use earnest_diagnostics::Problem;
    use std::collections::BTreeMap;
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
    fn a_rust_file_uses_the_library_however_the_path_is_spaced_or_grouped() {
        let folder = folder_with(&[
            ("plain.rs", "use earnest::model::Model;\n"),
            ("grouped.rs", "use earnest::{model::{Model, model}};\n"),
            ("spaced.rs", "use earnest :: model :: Model;\n"),
            ("broken.rs", "use earnest::\n    model::Model;\n"),
            ("renamed.rs", "use earnest as kit;\n"),
            ("external.rs", "extern crate earnest;\n"),
            ("whole.rs", "fn f() -> earnest::model::Timer { todo() }\n"),
            ("other.rs", "use earnest_rs::read;\n"),
            ("word.rs", "pub const MOOD: &str = \"earnest\";\n"),
        ]);
        assert_eq!(
            names_under(folder.path(), &[folder.path().to_path_buf()]),
            [
                "broken.rs",
                "external.rs",
                "grouped.rs",
                "plain.rs",
                "renamed.rs",
                "spaced.rs",
                "whole.rs",
            ],
            "every way to reach the library is a model file; another crate and the bare word \
             are not"
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

    fn unread_under(folder: &Path, roots: &[PathBuf]) -> Vec<(String, Problem)> {
        discover(roots)
            .expect("every root exists")
            .into_iter()
            .filter_map(|found| match found.text {
                Text::Unread(problem) => Some((
                    Path::new(&found.path)
                        .strip_prefix(folder)
                        .expect("a found file is under its root")
                        .to_string_lossy()
                        .into_owned(),
                    problem,
                )),
                Text::Model { .. } => None,
            })
            .collect()
    }

    fn places(unread: &[(String, Problem)]) -> Vec<(&str, u32, u32, &str)> {
        unread
            .iter()
            .map(|(path, problem)| {
                (
                    path.as_str(),
                    problem.line,
                    problem.column,
                    problem.feature.as_str(),
                )
            })
            .collect()
    }

    fn with_a_latin1_byte(text: &str) -> Vec<u8> {
        let mut bytes = text.as_bytes().to_vec();
        bytes.extend([0xE9, b'"', b';', b'\n']);
        bytes
    }

    #[test]
    fn a_model_file_with_a_byte_that_is_not_utf8_is_unread_at_that_byte() {
        let folder = folder_with(&[("fine.model.ts", TYPESCRIPT)]);
        let source = format!("{TYPESCRIPT}const a = \"caf");
        for name in ["stray.model.ts", "stray.rs"] {
            let mark = if name == "stray.rs" { RUST } else { "" };
            let bytes = with_a_latin1_byte(&format!("{mark}{source}"));
            fs::write(folder.path().join(name), bytes).expect("a file can be written");
        }
        let root = folder.path().to_path_buf();
        let unread = unread_under(folder.path(), std::slice::from_ref(&root));
        assert_eq!(
            places(&unread),
            [
                ("stray.model.ts", 2, 15, "`byte 0xE9`"),
                ("stray.rs", 3, 15, "`byte 0xE9`"),
            ],
            "each file is kept, with the place of its first byte that is not UTF-8"
        );
        assert!(
            unread[0].1.message.contains("UTF-8") && unread[0].1.file.ends_with("stray.model.ts"),
            "the problem names the file and the encoding: {:?}",
            unread[0].1
        );
        assert_eq!(
            names_under(folder.path(), &[root.join("stray.model.ts")]),
            ["stray.model.ts"],
            "the file is kept when it is named on the command line too"
        );
    }

    #[test]
    fn a_file_that_is_not_utf8_and_does_not_use_the_library_is_no_model() {
        let folder = folder_with(&[]);
        fs::write(
            folder.path().join("legacy.ts"),
            with_a_latin1_byte("const a = \"caf"),
        )
        .expect("a file can be written");
        assert!(
            names_under(folder.path(), &[folder.path().to_path_buf()]).is_empty(),
            "nothing in the file says it is a model"
        );
    }

    #[test]
    fn a_model_file_saved_as_utf16_is_unread() {
        let folder = folder_with(&[]);
        let little: Vec<u8> = TYPESCRIPT
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        let big: Vec<u8> = TYPESCRIPT
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect();
        for (name, mark, text) in [
            ("little.model.ts", [0xFF, 0xFE], little),
            ("big.model.ts", [0xFE, 0xFF], big),
        ] {
            let mut bytes = mark.to_vec();
            bytes.extend(text);
            fs::write(folder.path().join(name), bytes).expect("a file can be written");
        }
        let unread = unread_under(folder.path(), &[folder.path().to_path_buf()]);
        assert_eq!(
            places(&unread),
            [
                ("big.model.ts", 1, 1, "`UTF-16`"),
                ("little.model.ts", 1, 1, "`UTF-16`"),
            ],
            "the import is there once the file is decoded, so the file is a model"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_to_nothing_named_like_a_model_file_is_unread_and_a_link_to_a_folder_is_skipped() {
        use std::os::unix::fs::symlink;
        let folder = folder_with(&[("real/a.ts", TYPESCRIPT)]);
        let root = folder.path();
        symlink(root.join("nowhere.ts"), root.join("gone.model.ts")).expect("a link can be made");
        symlink(root.join("real"), root.join("linked.ts")).expect("a link can be made");
        symlink(root.join("real/a.ts"), root.join("same.ts")).expect("a link can be made");
        symlink(root.join("nowhere.md"), root.join("notes.md")).expect("a link can be made");
        assert_eq!(
            names_under(root, &[root.to_path_buf()]),
            ["gone.model.ts", "real/a.ts", "same.ts"],
            "the link to a file is read, the link to a folder is not followed"
        );
        let unread = unread_under(root, &[root.to_path_buf()]);
        assert_eq!(
            places(&unread),
            [("gone.model.ts", 1, 1, "`gone.model.ts`")],
            "only the link to nothing, at the top of the file and by its name"
        );
        assert!(
            unread[0].1.message.starts_with("the file can't be opened"),
            "the sentence says why: {}",
            unread[0].1.message
        );
    }

    #[test]
    fn a_folder_named_like_a_model_file_is_searched_as_a_folder() {
        let folder = folder_with(&[("odd.model.ts/a.ts", TYPESCRIPT)]);
        assert_eq!(
            names_under(folder.path(), &[folder.path().to_path_buf()]),
            ["odd.model.ts/a.ts"],
            "the folder is no file, and the model inside it is found"
        );
        assert_eq!(
            names_under(folder.path(), &[folder.path().join("odd.model.ts")]),
            ["odd.model.ts/a.ts"],
            "the same when the folder is named on the command line"
        );
        assert!(
            unread_under(folder.path(), &[folder.path().to_path_buf()]).is_empty(),
            "nothing is unread"
        );
    }

    #[test]
    fn a_folder_that_cant_be_listed_is_unread_under_its_own_path() {
        let folder = folder_with(&[]);
        let missing = folder.path().join("locked");
        let mut found = BTreeMap::new();
        walk(&missing, Path::new("src/locked"), &mut found);
        let all: Vec<&Found> = found.values().collect();
        assert_eq!(all.len(), 1, "the folder itself: {all:?}");
        assert_eq!(all[0].path, "src/locked", "the path it was reached by");
        let Text::Unread(problem) = &all[0].text else {
            panic!("the folder is unread, not {:?}", all[0].text);
        };
        assert_eq!(problem.feature, "`locked`", "the folder's name");
        assert!(
            problem.message.starts_with("the folder can't be opened"),
            "the sentence says why: {}",
            problem.message
        );
    }
}
