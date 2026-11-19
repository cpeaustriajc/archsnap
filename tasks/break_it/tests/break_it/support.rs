use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use serde_json::Value;
use tempfile::TempDir;

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
        let library = repository().join("npm/model/src/index.ts");
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

    pub(crate) fn commit(&self) {
        for arguments in [
            ["init", "--quiet", "."].as_slice(),
            ["add", "--all"].as_slice(),
            [
                "-c",
                "user.name=break it",
                "-c",
                "user.email=break-it@example.com",
                "commit",
                "--quiet",
                "--message",
                "before",
            ]
            .as_slice(),
        ] {
            let done = Command::new("git")
                .args(arguments)
                .current_dir(self.path())
                .output()
                .expect("`git` is on the `PATH`");
            assert!(
                done.status.success(),
                "git {arguments:?} failed:\n{}",
                String::from_utf8_lossy(&done.stderr)
            );
        }
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
    pub(crate) fn is_unreadable(&self) -> bool {
        self.json["unreadable"]
            .as_array()
            .is_some_and(|files| !files.is_empty())
    }

    pub(crate) fn model(&self, name: &str) -> &Value {
        self.json["models"]
            .as_array()
            .and_then(|models| models.iter().find(|model| model["name"] == name))
            .unwrap_or(&Value::Null)
    }

    pub(crate) fn change(&self, id: &str) -> &Value {
        self.json["ruleChanges"]
            .as_array()
            .and_then(|changes| changes.iter().find(|change| change["id"] == id))
            .unwrap_or(&Value::Null)
    }
}

pub(crate) fn rule<'a>(model: &'a Value, id: &str) -> &'a Value {
    model["rules"]
        .as_array()
        .and_then(|rules| rules.iter().find(|rule| rule["id"] == id))
        .unwrap_or(&Value::Null)
}

pub(crate) fn is_broken_after(rule: &Value, steps: usize) -> bool {
    rule["verdict"] == "broken"
        && rule["path"]["steps"]
            .as_array()
            .is_some_and(|taken| taken.len() == steps)
}
