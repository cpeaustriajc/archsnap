use earnest_diagnostics::{Place, Problem};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::Utf8Error;

const SKIPPED_FOLDERS: [&str; 4] = ["node_modules", "target", "dist", "build"];
const TYPESCRIPT_EXTENSIONS: [&str; 3] = ["ts", "mts", "tsx"];
const MODEL_SUFFIXES: [&str; 3] = [".model.ts", ".model.mts", ".model.tsx"];
const TYPESCRIPT_TEST_SUFFIXES: [&str; 6] = [
    ".test.ts",
    ".test.mts",
    ".test.tsx",
    ".spec.ts",
    ".spec.mts",
    ".spec.tsx",
];
const TYPESCRIPT_TEST_FOLDERS: [&str; 3] = ["test", "tests", "__tests__"];
const RUST_TEST_FOLDERS: [&str; 3] = ["tests", "benches", "examples"];
const RUST_BUILD_SCRIPT: &str = "build.rs";
const RUST_EXTENSION: &str = "rs";
const HERE: &str = ".";
pub(crate) const IN_EVERY_MODEL_FILE: &str = "earnest";
const UTF16_LITTLE_ENDIAN: [u8; 2] = [0xFF, 0xFE];
const UTF16_BIG_ENDIAN: [u8; 2] = [0xFE, 0xFF];
const NOT_UTF8: &str = "the file is not UTF-8 text, so the checker can't read it: save it as UTF-8";
const TOP_OF_FILE: Place = Place { line: 1, column: 1 };

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Origin {
    Named,
    Searched,
}

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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Discovered {
    pub(crate) found: Vec<Found>,
    pub(crate) skipped: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Searched {
    Found(Found),
    Skipped,
}

#[derive(Debug, Default)]
struct Walked {
    found: BTreeMap<PathBuf, Found>,
    skipped: BTreeSet<PathBuf>,
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
    let name = path.file_name().and_then(OsStr::to_str)?;
    let (_, extension) = name.rsplit_once('.')?;
    if TYPESCRIPT_EXTENSIONS.contains(&extension) {
        Some(Language::TypeScript)
    } else if extension == RUST_EXTENSION {
        Some(Language::Rust)
    } else {
        None
    }
}

pub(crate) fn is_model_source(language: Language, path: &str, source: &str) -> bool {
    let reaches = |text: &str| match language {
        Language::TypeScript => earnest_ts::reaches_library(path, text),
        Language::Rust => earnest_rs::uses_library(text),
    };
    reaches(source) || (source.contains('\0') && reaches(&source.replace('\0', "")))
}

pub(crate) fn is_model_name(path: &str) -> bool {
    let name = Path::new(path)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or_default();
    MODEL_SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

pub(crate) fn is_test_file(language: Language, below: &Path) -> bool {
    let name = below
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or_default();
    let in_folder = |folders: &[&str]| {
        below.parent().is_some_and(|parent| {
            parent
                .components()
                .any(|folder| folders.iter().any(|test| folder.as_os_str() == *test))
        })
    };
    match language {
        Language::TypeScript => {
            TYPESCRIPT_TEST_SUFFIXES
                .iter()
                .any(|suffix| name.ends_with(suffix))
                || in_folder(&TYPESCRIPT_TEST_FOLDERS)
        }
        Language::Rust => name == RUST_BUILD_SCRIPT || in_folder(&RUST_TEST_FOLDERS),
    }
}

pub(crate) fn is_model_file(language: Language, path: &str, source: &str) -> bool {
    is_model_name(path) || is_model_source(language, path, source)
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
    folders_are_searched
        && language_of(below).is_some_and(|language| {
            is_model_name(&below.to_string_lossy()) || !is_test_file(language, below)
        })
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

fn not_utf8(
    path: &Path,
    language: Language,
    bytes: &[u8],
    error: Utf8Error,
    origin: Origin,
) -> Option<Problem> {
    let shown = path.to_string_lossy();
    let certain = origin == Origin::Named || is_model_name(&shown);
    let in_utf16 =
        utf16_text(bytes).is_some_and(|decoded| is_model_source(language, &shown, &decoded));
    if in_utf16 {
        return Some(Problem::new(&shown, TOP_OF_FILE, "`UTF-16`", NOT_UTF8));
    }
    let lossy = String::from_utf8_lossy(bytes);
    if !certain && !is_model_source(language, &shown, &lossy) {
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

fn not_a_known_language(path: &Path) -> Found {
    Found::unread(
        path,
        Problem::new(
            &path.to_string_lossy(),
            TOP_OF_FILE,
            &in_backticks(path),
            "the file was named on the command line, but it is not TypeScript (`.ts`, `.mts`, \
             `.tsx`) or Rust (`.rs`), so the checker does not know how to read it",
        ),
    )
}

fn model_file(path: &Path, origin: Origin, below: &Path) -> Option<Searched> {
    let Some(language) = language_of(path) else {
        return (origin == Origin::Named).then(|| Searched::Found(not_a_known_language(path)));
    };
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return Some(Searched::Found(Found::unread(
                path,
                cant_be_opened(path, "file", &error),
            )));
        }
    };
    let shown = path.to_string_lossy();
    let is_test =
        origin == Origin::Searched && !is_model_name(&shown) && is_test_file(language, below);
    let found = match String::from_utf8(bytes) {
        Ok(source) => {
            if origin == Origin::Named || is_model_file(language, &shown, &source) {
                Found::model(&shown, language, &source)
            } else {
                return None;
            }
        }
        Err(refused) => {
            let error = refused.utf8_error();
            let problem = not_utf8(path, language, refused.as_bytes(), error, origin)?;
            Found::unread(path, problem)
        }
    };
    Some(if is_test {
        Searched::Skipped
    } else {
        Searched::Found(found)
    })
}

fn keep(walked: &mut Walked, shown: PathBuf, searched: Option<Searched>) {
    match searched {
        Some(Searched::Found(found)) => {
            walked.found.insert(shown, found);
        }
        Some(Searched::Skipped) => {
            walked.skipped.insert(shown);
        }
        None => {}
    }
}

fn walk(folder: &Path, shown: &Path, below: &Path, walked: &mut Walked) {
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
            walked
                .found
                .insert(named.to_path_buf(), Found::unread(named, problem));
            return;
        }
    };
    for entry in entries {
        let name = entry.file_name();
        let shown_entry = shown.join(&name);
        let below_entry = below.join(&name);
        let Ok(kind) = entry.file_type() else {
            let searched = model_file(&shown_entry, Origin::Searched, &below_entry);
            keep(walked, shown_entry, searched);
            continue;
        };
        if kind.is_dir() {
            if !is_skipped_folder(&name) {
                walk(&entry.path(), &shown_entry, &below_entry, walked);
            }
        } else if kind.is_file() || (kind.is_symlink() && !entry.path().is_dir()) {
            let searched = model_file(&shown_entry, Origin::Searched, &below_entry);
            keep(walked, shown_entry, searched);
        }
    }
}

pub(crate) fn discover(roots: &[PathBuf]) -> Result<Discovered, String> {
    let here = [PathBuf::from(HERE)];
    let roots = if roots.is_empty() { &here } else { roots };
    let mut walked = Walked::default();
    let mut named = BTreeSet::new();
    for root in roots {
        if root.is_dir() {
            let shown = if root == Path::new(HERE) {
                Path::new("")
            } else {
                root.as_path()
            };
            walk(root, shown, Path::new(""), &mut walked);
        } else if root.is_file() {
            let canonical = fs::canonicalize(root).unwrap_or_else(|_| root.clone());
            if named.insert(canonical) {
                let searched = model_file(root, Origin::Named, Path::new(""));
                keep(&mut walked, root.clone(), searched);
            }
        } else {
            return Err(format!(
                "`{}` is not a file or a folder",
                root.to_string_lossy()
            ));
        }
    }
    let skipped = walked
        .skipped
        .iter()
        .filter(|path| {
            let canonical = fs::canonicalize(path).unwrap_or_else(|_| (*path).clone());
            !walked.found.contains_key(*path) && !named.contains(&canonical)
        })
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    Ok(Discovered {
        found: walked.found.into_values().collect(),
        skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::{Found, Language, Text, Walked, discover, is_test_file, language_of, walk};
    use earnest_diagnostics::Problem;
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
            .found
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
            .found
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

    #[test]
    fn a_typescript_file_is_a_model_file_by_what_its_import_means_not_by_how_it_is_written() {
        let folder = folder_with(&[
            (
                "hex.model.ts",
                "import { m } from \"@earnest\\x2fmodel\";\n",
            ),
            (
                "unicode.model.ts",
                "import { m } from \"\\u0040earnest/model\";\n",
            ),
            (
                "continued.model.ts",
                "import { m } from \"@earnest/mo\\\ndel\";\n",
            ),
            ("require.ts", "const m = require(\"@earnest/model\");\n"),
            ("other.ts", "import { m } from \"@earnest\\x2fmodels\";\n"),
        ]);
        assert_eq!(
            names_under(folder.path(), &[folder.path().to_path_buf()]),
            [
                "continued.model.ts",
                "hex.model.ts",
                "require.ts",
                "unicode.model.ts"
            ],
            "an escape or a line continuation in the specifier changes nothing"
        );
        assert_eq!(
            names_under(folder.path(), &[folder.path().join("hex.model.ts")]),
            ["hex.model.ts"],
            "the same when the file is named on the command line"
        );
    }

    #[test]
    fn a_rust_file_is_a_model_file_whatever_lies_between_the_segments_of_its_path() {
        let folder = folder_with(&[
            (
                "block.rs",
                "use earnest::/* the model module */model::{Model};\n",
            ),
            (
                "line.rs",
                "use earnest:: // the model module\n    model::Model;\n",
            ),
            ("quiet.rs", "// use earnest::model::Model;\nfn f() {}\n"),
        ]);
        assert_eq!(
            names_under(folder.path(), &[folder.path().to_path_buf()]),
            ["block.rs", "line.rs"],
            "a comment inside the path is nothing, and a path in a comment is not a use"
        );
    }

    #[test]
    fn a_file_that_starts_with_a_utf16_mark_and_holds_ascii_text_is_unread_at_its_first_byte() {
        let folder = folder_with(&[]);
        for (name, mark) in [
            ("little.model.ts", [0xFF, 0xFE]),
            ("big.model.ts", [0xFE, 0xFF]),
        ] {
            let mut bytes = mark.to_vec();
            bytes.extend(TYPESCRIPT.as_bytes());
            fs::write(folder.path().join(name), bytes).expect("a file can be written");
        }
        let unread = unread_under(folder.path(), &[folder.path().to_path_buf()]);
        assert_eq!(
            places(&unread),
            [
                ("big.model.ts", 1, 1, "`byte 0xFE`"),
                ("little.model.ts", 1, 1, "`byte 0xFF`"),
            ],
            "the text is plain, so the first byte is the one that is not UTF-8, as a UTF-16 \
             decoder would find nonsense and drop the file"
        );
    }

    #[test]
    fn a_file_of_text_with_a_null_between_every_letter_is_a_model_file_that_is_read() {
        let folder = folder_with(&[]);
        let spaced: String = TYPESCRIPT
            .chars()
            .flat_map(|letter| [letter, '\0'])
            .collect();
        fs::write(folder.path().join("spaced.model.ts"), spaced).expect("a file can be written");
        assert_eq!(
            names_under(folder.path(), &[folder.path().to_path_buf()]),
            ["spaced.model.ts"],
            "it is kept, and the reader then refuses the nulls"
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
        let mut walked = Walked::default();
        walk(
            &missing,
            Path::new("src/locked"),
            Path::new("locked"),
            &mut walked,
        );
        let all: Vec<&Found> = walked.found.values().collect();
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

    const JOINED: &str =
        "const { m } = await import(\"@ear\" + \"nest/model\");\nexport const one = 1;\n";

    #[test]
    fn a_model_named_file_is_a_model_file_whatever_it_imports() {
        let folder = folder_with(&[
            ("joined.model.ts", JOINED),
            ("joined.ts", JOINED),
            ("plain.model.mts", "export const one = 1;\n"),
        ]);
        assert_eq!(
            names_under(folder.path(), &[folder.path().to_path_buf()]),
            ["joined.model.ts", "plain.model.mts"],
            "the `.model.` name decides; a plain name that hides its import is not found"
        );
    }

    #[test]
    fn a_file_named_on_the_command_line_is_a_model_file_whatever_it_imports() {
        let folder = folder_with(&[("joined.ts", JOINED), ("plain.rs", "pub fn one() {}\n")]);
        let roots = [
            folder.path().join("joined.ts"),
            folder.path().join("plain.rs"),
        ];
        assert_eq!(
            names_under(folder.path(), &roots),
            ["joined.ts", "plain.rs"],
            "both are read"
        );
    }

    #[test]
    fn a_name_that_is_only_an_extension_is_a_file_of_that_language() {
        for (name, language) in [
            (".ts", Language::TypeScript),
            (".mts", Language::TypeScript),
            (".tsx", Language::TypeScript),
            (".rs", Language::Rust),
        ] {
            assert_eq!(language_of(Path::new(name)), Some(language), "{name}");
        }
        let folder = folder_with(&[(".ts", JOINED)]);
        assert_eq!(
            names_under(folder.path(), &[folder.path().join(".ts")]),
            [".ts"],
            "named, it is read"
        );
    }

    #[test]
    fn a_named_file_of_no_known_language_is_unreadable_with_a_sentence() {
        let folder = folder_with(&[("notes.md", "# notes\n")]);
        let unread = unread_under(folder.path(), &[folder.path().join("notes.md")]);
        assert_eq!(unread.len(), 1, "{unread:?}");
        assert!(
            unread[0].1.message.contains("does not know how to read it"),
            "{}",
            unread[0].1.message
        );
        assert!(
            names_under(folder.path(), &[folder.path().to_path_buf()]).is_empty(),
            "found in a folder, it is not a model file"
        );
    }

    #[test]
    fn the_same_file_under_two_spellings_is_listed_once() {
        let folder = folder_with(&[("a.model.ts", TYPESCRIPT)]);
        let root = folder.path();
        let direct = root.join("a.model.ts");
        let roundabout = root.join("sub/../a.model.ts");
        fs::create_dir_all(root.join("sub")).expect("a folder");
        let found = discover(&[direct, roundabout]).expect("both exist").found;
        assert_eq!(found.len(), 1, "one file: {found:?}");
    }

    #[test]
    fn a_model_named_file_that_is_not_utf8_is_unreadable() {
        let folder = folder_with(&[]);
        fs::write(folder.path().join("odd.model.ts"), [b'a', 0xE9, b';']).expect("a file");
        let unread = unread_under(folder.path(), &[folder.path().to_path_buf()]);
        assert_eq!(unread.len(), 1, "{unread:?}");
    }

    fn skipped_under(folder: &Path, roots: &[PathBuf]) -> Vec<String> {
        discover(roots)
            .expect("every root exists")
            .skipped
            .into_iter()
            .map(|path| {
                Path::new(&path)
                    .strip_prefix(folder)
                    .expect("a skipped file is under its root")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    fn app_with_tests() -> tempfile::TempDir {
        folder_with(&[
            ("src/a.model.ts", TYPESCRIPT),
            ("src/b.ts", TYPESCRIPT),
            ("src/b.test.ts", TYPESCRIPT),
            ("src/b.spec.mts", TYPESCRIPT),
            ("src/c.test.tsx", TYPESCRIPT),
            ("src/plain.test.ts", "export const one = 1;\n"),
            ("tests/x.ts", TYPESCRIPT),
            ("tests/kept.model.ts", TYPESCRIPT),
            ("src/__tests__/y.tsx", TYPESCRIPT),
            ("test/z.ts", TYPESCRIPT),
            ("src/contest.ts", TYPESCRIPT),
            ("src/testing/t.ts", TYPESCRIPT),
            ("examples/e.ts", TYPESCRIPT),
            ("src/lib.rs", RUST),
            ("src/tests.rs", RUST),
            ("tests/it.rs", RUST),
            ("benches/speed.rs", RUST),
            ("crates/a/examples/demo.rs", RUST),
            ("build.rs", RUST),
            ("crates/a/build.rs", RUST),
            ("test/singular.rs", RUST),
        ])
    }

    #[test]
    fn a_test_file_that_uses_the_library_is_skipped_and_listed_and_a_model_name_always_wins() {
        let folder = app_with_tests();
        let roots = [folder.path().to_path_buf()];
        assert_eq!(
            names_under(folder.path(), &roots),
            [
                "examples/e.ts",
                "src/a.model.ts",
                "src/b.ts",
                "src/contest.ts",
                "src/lib.rs",
                "src/testing/t.ts",
                "src/tests.rs",
                "test/singular.rs",
                "tests/kept.model.ts",
            ],
            "the test conventions of each language, and a `.model.ts` name wherever it is"
        );
        assert_eq!(
            skipped_under(folder.path(), &roots),
            [
                "benches/speed.rs",
                "build.rs",
                "crates/a/build.rs",
                "crates/a/examples/demo.rs",
                "src/__tests__/y.tsx",
                "src/b.spec.mts",
                "src/b.test.ts",
                "src/c.test.tsx",
                "test/z.ts",
                "tests/it.rs",
                "tests/x.ts",
            ],
            "every test file that uses the library, in path order; one that does not is no one's \
             business"
        );
    }

    #[test]
    fn a_file_named_on_the_command_line_is_read_whatever_its_name_or_folder() {
        let folder = app_with_tests();
        let root = folder.path();
        let roots = [
            root.join("tests/x.ts"),
            root.join("src/b.test.ts"),
            root.join("build.rs"),
        ];
        assert_eq!(
            names_under(root, &roots),
            ["build.rs", "src/b.test.ts", "tests/x.ts"],
            "all three are read"
        );
        assert!(
            skipped_under(root, &roots).is_empty(),
            "none of them is skipped"
        );
    }

    #[test]
    fn a_file_both_named_and_found_in_a_searched_folder_is_read_not_skipped() {
        let folder = app_with_tests();
        let root = folder.path();
        let roots = [root.join("src"), root.join("src/b.test.ts")];
        assert!(
            names_under(root, &roots).contains(&"src/b.test.ts".to_owned()),
            "the named file is read"
        );
        assert!(
            !skipped_under(root, &roots).contains(&"src/b.test.ts".to_owned()),
            "and not listed as skipped"
        );
    }

    #[test]
    fn a_folder_named_on_the_command_line_is_judged_by_the_names_below_it() {
        let folder = app_with_tests();
        let root = folder.path();
        let roots = [root.join("tests")];
        assert_eq!(
            names_under(root, &roots),
            ["tests/it.rs", "tests/kept.model.ts", "tests/x.ts"],
            "the folder the user named is not itself a test folder"
        );
        assert!(
            skipped_under(root, &roots).is_empty(),
            "nothing below it is a test file by its own folders"
        );
    }

    #[test]
    fn a_test_file_that_is_not_utf8_and_uses_the_library_is_skipped() {
        let folder = folder_with(&[]);
        fs::create_dir_all(folder.path().join("tests")).expect("a folder");
        fs::write(
            folder.path().join("tests/legacy.test.ts"),
            with_a_latin1_byte(&format!("{TYPESCRIPT}const a = \"caf")),
        )
        .expect("a file can be written");
        let roots = [folder.path().to_path_buf()];
        assert_eq!(
            skipped_under(folder.path(), &roots),
            ["tests/legacy.test.ts"]
        );
        assert!(
            names_under(folder.path(), &roots).is_empty(),
            "it is not read"
        );
    }

    #[test]
    fn the_convention_is_by_language() {
        for (name, language, expected) in [
            ("a.test.ts", Language::TypeScript, true),
            ("a.spec.tsx", Language::TypeScript, true),
            ("a.test.mts", Language::TypeScript, true),
            ("a.model.test.ts", Language::TypeScript, true),
            ("test/a.ts", Language::TypeScript, true),
            ("x/tests/a.ts", Language::TypeScript, true),
            ("x/__tests__/deep/a.ts", Language::TypeScript, true),
            ("examples/a.ts", Language::TypeScript, false),
            ("benches/a.ts", Language::TypeScript, false),
            ("a.tests.ts", Language::TypeScript, false),
            ("tests.ts", Language::TypeScript, false),
            ("build.rs", Language::TypeScript, false),
            ("tests/a.rs", Language::Rust, true),
            ("x/benches/a.rs", Language::Rust, true),
            ("x/examples/deep/a.rs", Language::Rust, true),
            ("x/build.rs", Language::Rust, true),
            ("test/a.rs", Language::Rust, false),
            ("__tests__/a.rs", Language::Rust, false),
            ("a.test.rs", Language::Rust, false),
            ("tests.rs", Language::Rust, false),
            ("build/a.rs", Language::Rust, false),
        ] {
            assert_eq!(
                is_test_file(language, Path::new(name)),
                expected,
                "{name} as {language:?}"
            );
        }
    }
}
