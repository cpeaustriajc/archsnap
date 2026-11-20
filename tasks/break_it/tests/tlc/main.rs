//! The tests that need Java and the TLC jar, built only with the cargo feature `tlc`:
//! `cargo test --locked -p earnest_break_it --features tlc --test tlc`. The jar is the one
//! `just tlc` puts at `target/tools/tla2tools.jar`, or the path in `EARNEST_TLC_JAR`.

use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use tempfile::TempDir;

const TLC_DEADLINE: Duration = Duration::from_mins(10);
const POLL: Duration = Duration::from_millis(50);
const TLC_JAR_VARIABLE: &str = "EARNEST_TLC_JAR";
const LIMIT_OF_NUMBERS: &str = "TLC's integers hold 32 bits";
const LIMIT_OF_TEXTS: &str = "which is not ASCII";
const TLC_JAR_FROM_THE_ROOT: &str = "../../target/tools/tla2tools.jar";

fn binary() -> PathBuf {
    let test = env::current_exe().expect("a test knows where its own binary is");
    let folder = test
        .parent()
        .and_then(Path::parent)
        .expect("a test binary sits in `deps` under the folder of its build profile");
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut build = Command::new(cargo);
    build
        .args(["build", "--locked", "--package", "earnest-cli"])
        .args(["--bin", "earnest"])
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    if folder.file_name().is_some_and(|name| name == "release") {
        build.arg("--release");
    }
    let compiled = build.output().expect("cargo starts");
    assert!(
        compiled.status.success(),
        "the earnest binary does not build:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    folder.join(format!("earnest{}", env::consts::EXE_SUFFIX))
}

struct Finished {
    stdout: String,
    success: bool,
}

fn tlc_jar() -> PathBuf {
    env::var_os(TLC_JAR_VARIABLE).map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join(TLC_JAR_FROM_THE_ROOT),
        PathBuf::from,
    )
}

fn run_for(files: &[(&str, &str)], arguments: &[&str], deadline: Duration) -> Option<Finished> {
    let binary = binary();
    let folder = TempDir::new().expect("a temporary folder can be made");
    for (name, text) in files {
        std::fs::write(folder.path().join(name), text).expect("a file can be written");
    }
    let mut child = Command::new(binary)
        .args(arguments)
        .current_dir(folder.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("earnest starts");
    let started = Instant::now();
    while started.elapsed() < deadline {
        if child.try_wait().expect("the child can be polled").is_some() {
            let output = child.wait_with_output().expect("the output can be read");
            return Some(Finished {
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                success: output.status.success(),
            });
        }
        thread::sleep(POLL);
    }
    child.kill().expect("the child can be stopped");
    child.wait().expect("the child can be reaped");
    None
}

fn assert_tlc_cannot_confirm(name: &str, source: &str, limit: &str, why: &str) {
    let jar = tlc_jar();
    assert!(
        jar.is_file(),
        "TLC is missing at {}: `just tlc` downloads it to target/tools, and {TLC_JAR_VARIABLE} \
         names another copy",
        jar.display()
    );
    let jar = jar.to_string_lossy().into_owned();
    let confirmed = run_for(
        &[(name, source)],
        &["confirm", "--tlc", &jar, name],
        TLC_DEADLINE,
    )
    .expect("TLC checks a model of a few states in minutes");
    assert!(
        !confirmed.success
            && confirmed.stdout.starts_with("cannot")
            && confirmed.stdout.contains("TLC can't confirm this model")
            && confirmed.stdout.contains(limit),
        "the contract says `confirm` prints `cannot` and names the limit of TLC ({limit}) for a \
         model TLC can't hold, and `{name}` is a model {why}\n{}",
        confirmed.stdout
    );
}

#[test]
fn confirm_says_tlc_cannot_hold_a_number_beyond_32_bits() {
    assert_tlc_cannot_confirm(
        "big.model.ts",
        include_str!("../../models/fourth_b/big_number.model.ts"),
        LIMIT_OF_NUMBERS,
        "whose numbers the search holds exactly (the largest is 2^31 + 2) and TLC's 32-bit \
         integers can't",
    );
}

#[test]
fn confirm_says_tlc_cannot_hold_a_product_past_32_bits_of_numbers_within_them() {
    assert_tlc_cannot_confirm(
        "square.model.ts",
        include_str!("../../models/fourth_b/square.model.ts"),
        LIMIT_OF_NUMBERS,
        "whose numbers are all below 2^31 and whose largest square, 50 003 times itself, is \
         2 500 300 009",
    );
}

#[test]
fn confirm_says_tlc_cannot_tell_two_texts_above_ascii_apart() {
    assert_tlc_cannot_confirm(
        "text.model.ts",
        include_str!("../../models/fourth_b/text_collision.model.ts"),
        LIMIT_OF_TEXTS,
        "with two states, the letters `Z` (U+005A) and `\u{15a}` (U+015A), which TLC takes for \
         one when it counts states",
    );
}

const HOOKS: &str = r#"import { registerHooks } from "node:module";
import { pathToFileURL } from "node:url";

const library = pathToFileURL(process.env.EARNEST_MODEL_LIBRARY).href;

registerHooks({
	resolve(specifier, context, nextResolve) {
		if (specifier === "@earnest/model") return { url: library, shortCircuit: true };
		return nextResolve(specifier, context);
	},
});
"#;

const EXPLORE: &str = r#"import { explore } from "@earnest/model";

const [modelFile, modelName] = process.argv.slice(2);
const models = await import(modelFile);
console.log(JSON.stringify(explore(models[modelName], { exports: models })));
"#;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root is two folders above this crate")
}

fn profile_folder() -> PathBuf {
    let test = env::current_exe().expect("a test knows where its own binary is");
    test.parent()
        .and_then(Path::parent)
        .expect("a test binary sits in `deps` under the folder of its build profile")
        .to_path_buf()
}

fn earnest() -> &'static Path {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY.get_or_init(|| {
        let folder = profile_folder();
        let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut build = Command::new(cargo);
        build
            .args(["build", "--locked", "--package", "earnest-cli"])
            .args(["--bin", "earnest"])
            .current_dir(repository());
        if folder.file_name().is_some_and(|name| name == "release") {
            build.arg("--release");
        }
        let compiled = build.output().expect("cargo starts");
        assert!(
            compiled.status.success(),
            "the earnest binary does not build:\n{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let binary = folder.join(format!("earnest{}", env::consts::EXE_SUFFIX));
        assert!(
            binary.is_file(),
            "the earnest binary is missing at {}",
            binary.display()
        );
        binary
    })
}

pub(crate) struct Sandbox {
    folder: TempDir,
}

impl Sandbox {
    pub(crate) fn with(files: &[(&str, &str)]) -> Self {
        let folder = tempfile::Builder::new()
            .prefix("earnest-break-it-")
            .tempdir()
            .expect("a temporary folder can be made");
        let sandbox = Self { folder };
        for (name, text) in files {
            sandbox.write(name, text);
        }
        sandbox
    }

    fn path(&self) -> &Path {
        self.folder.path()
    }

    pub(crate) fn write(&self, name: &str, text: &str) {
        fs::write(self.path().join(name), text).expect("a file in the sandbox can be written");
    }

    fn finished(&self, command: &mut Command, what: &str) -> Report {
        let output = command
            .current_dir(self.path())
            .output()
            .unwrap_or_else(|error| panic!("{what} does not start: {error}"));
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let json = serde_json::from_str(&stdout).unwrap_or_else(|error| {
            panic!("{what} did not print JSON ({error}):\n{stdout}\n{stderr}")
        });
        Report { json, stderr }
    }

    pub(crate) fn verify(&self, arguments: &[&str]) -> Report {
        let mut command = Command::new(earnest());
        command
            .args(["verify", "--json", "--no-cache"])
            .args(arguments);
        self.finished(&mut command, "earnest verify")
    }

    pub(crate) fn explore(&self, file: &str, model: &str) -> Report {
        let library = repository().join("packages/model/src/index.ts");
        assert!(
            library.is_file(),
            "the TypeScript model library is missing at {}",
            library.display()
        );
        self.write("hooks.mjs", HOOKS);
        self.write("explore.mjs", EXPLORE);
        let mut command = Command::new("node");
        command
            .args(["--import", "./hooks.mjs", "explore.mjs"])
            .args([&format!("./{file}"), model])
            .env("EARNEST_MODEL_LIBRARY", &library);
        self.finished(&mut command, "node")
    }
}

pub(crate) struct Report {
    pub(crate) json: Value,
    stderr: String,
}

impl fmt::Display for Report {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:#}\n{}", self.json, self.stderr)
    }
}

impl Report {
    pub(crate) fn model(&self, name: &str) -> &Value {
        self.json["models"]
            .as_array()
            .and_then(|models| models.iter().find(|model| model["name"] == name))
            .unwrap_or(&Value::Null)
    }
}

pub(crate) fn rule<'a>(model: &'a Value, id: &str) -> &'a Value {
    model["rules"]
        .as_array()
        .and_then(|rules| rules.iter().find(|rule| rule["id"] == id))
        .unwrap_or(&Value::Null)
}

struct Ran {
    stdout: String,
    stderr: String,
    code: Option<i32>,
}

impl Ran {
    fn said(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

fn run_in_folder(files: &[(&str, &str)], arguments: &[&str]) -> Ran {
    let binary = binary();
    let folder = TempDir::new().expect("a temporary folder can be made");
    for (name, text) in files {
        let path = folder.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("a folder in the sandbox can be made");
        }
        fs::write(path, text).expect("a file in the sandbox can be written");
    }
    let output = Command::new(binary)
        .args(arguments)
        .current_dir(folder.path())
        .output()
        .expect("earnest starts");
    Ran {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        code: output.status.code(),
    }
}

#[test]
fn tlc_judges_a_quantifier_that_an_early_value_decides_as_the_checker_does() {
    let jar = tlc_jar();
    assert!(
        jar.is_file(),
        "TLC is missing at {}: `just tlc` downloads it to target/tools, and {TLC_JAR_VARIABLE} \
         names another copy",
        jar.display()
    );
    let jar = jar.to_string_lossy().into_owned();
    let file = "sweep.model.ts";
    let source = include_str!("../../models/sixth/sweep.model.ts");
    let sandbox = Sandbox::with(&[(file, source)]);
    let checked = sandbox.verify(&[]);
    let explored = sandbox.explore(file, "Sweep");
    for (id, verdict) in [("wide", "broken"), ("some", "holds")] {
        assert_eq!(
            rule(checked.model("Sweep"), id)["verdict"],
            verdict,
            "{checked}"
        );
        assert_eq!(rule(&explored.json, id)["verdict"], verdict, "{explored}");
    }
    let confirmed = run_in_folder(&[(file, source)], &["confirm", "--tlc", &jar, file]);
    assert!(
        confirmed.code == Some(0) && confirmed.stdout.starts_with("same"),
        "`m.forall(Digit, (x) => m.div(6, 3 - x) > 3 + s.n)` is false at `x = 1`, so the \
         checker and Node stop there and `wide` is broken in the start state; `m.exists` with \
         `>= 3 - s.n` is true at `x = 1`, so `some` holds. `m.div(6, 0)` at `x = 3` is never \
         evaluated. The README says TLC finds the same verdict for each rule, but the module asks \
         that the body has a value for every `x`, so TLC says `unknown` for both and `confirm` \
         exits 1\n{}",
        confirmed.said()
    );
}
