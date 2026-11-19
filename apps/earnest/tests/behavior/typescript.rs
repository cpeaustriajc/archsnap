use std::process::Command;

use serde_json::json;

use crate::expected::{self, OTHER_TYPESCRIPT};
use crate::support::{Sandbox, repository};

#[test]
fn r01_every_typescript_model_passes_the_strict_type_checker() {
    let library = repository().join("npm/model/src/index.ts");
    assert!(
        library.is_file(),
        "the TypeScript model library is missing at {}",
        library.display()
    );
    let mut fixtures = expected::files();
    fixtures.extend(OTHER_TYPESCRIPT);
    let sandbox = Sandbox::with(&fixtures);
    let files: Vec<String> = fixtures
        .iter()
        .map(|fixture| format!("models/{fixture}"))
        .collect();
    let configuration = json!({
        "compilerOptions": {
            "strict": true,
            "noEmit": true,
            "target": "ES2022",
            "module": "ESNext",
            "moduleResolution": "bundler",
            "lib": ["ES2022", "DOM"],
            "types": [],
            "allowImportingTsExtensions": true,
            "forceConsistentCasingInFileNames": true,
            "paths": { "@earnest/model": [library] },
        },
        "files": files,
    });
    sandbox.write("tsconfig.json", &configuration.to_string());

    let output = Command::new("tsc")
        .args(["--project", "tsconfig.json", "--pretty", "false"])
        .current_dir(sandbox.path())
        .output()
        .expect("`tsc` is on the `PATH`");
    assert!(
        output.status.success(),
        "`tsc --noEmit` with `strict` rejects a model file:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn r01_the_checker_reads_every_typescript_model_the_type_checker_accepts() {
    let mut fixtures = expected::files();
    fixtures.extend([
        "changes/before/tank.model.ts",
        "changes/after/tank.model.ts",
    ]);
    let run = Sandbox::with(&fixtures).earnest(&["verify", "--json"]);
    run.exits(1);
    let report = run.json();
    assert_eq!(
        report["unreadable"],
        json!([]),
        "no model file is unreadable\n{run}"
    );
    assert_eq!(
        report["summary"]["unfinished"], 0,
        "every model is fully checked"
    );
}
