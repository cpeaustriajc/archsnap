# @earnest/cli

The `earnest` model checker, installed from npm. The repository's `README.md` is the contract for
what `earnest` does; this package only gets the binary onto your machine.

```sh
npm install --save-dev @earnest/cli @earnest/model
npx earnest verify
```

`pnpm add -D` and `yarn add -D` work the same way. The package puts `earnest` on the `PATH` of
your scripts and of `npx`.

## How it ships

The way oxlint and Biome ship a Rust binary through npm:

- `@earnest/cli` is a small Node program. Its `bin` entry `earnest` is a launcher that finds the
  binary and runs it, passing the arguments, the standard streams, the signals and the exit code
  through unchanged.
- The binary itself is in one package for each platform, listed in `optionalDependencies`:
  `@earnest/cli-linux-x64-gnu`, `@earnest/cli-linux-arm64-gnu`, `@earnest/cli-darwin-arm64`,
  `@earnest/cli-darwin-x64` and `@earnest/cli-win32-x64-msvc`. Each says which `os`, `cpu` and
  `libc` it is for, so a package manager installs only the one that fits the machine.
- There is no `postinstall` script and nothing is downloaded when the package is installed or when
  `earnest` first runs. After `npm install`, `earnest` needs no network, no Java and no setup step.

If no package fits the machine, the launcher prints one sentence naming the platform, such as
`earnest has no binary for linux-x64-musl, and @earnest/cli ships one only for ...`, and exits
with 127. It is also what you see when optional dependencies were left out of the install.
`EARNEST_BINARY` set to the path of a binary makes the launcher run that file instead; it is how
the launcher's own tests use a stand-in.

`earnest verify` is the authority on exit codes: 0 when every model passed, 1 when a rule broke
or a model did not finish, 2 for bad flags. The launcher hands those through, and a binary that
a signal killed makes the launcher die of the same signal.

## Building it

From `npm/cli`, after `pnpm install --frozen-lockfile`:

```sh
cargo build --release -p earnest-cli
node scripts/assemble.ts --out build/platform    # the package for this machine's platform
pnpm pack --pack-destination build               # in build/platform, and again here for the launcher
```

`scripts/assemble.ts` copies `target/release/earnest` into `<out>/bin/` with the manifest kept in
`platforms/<platform>/package.json`. `pnpm pack` in `npm/cli` builds the launcher into `dist/`
first. `pnpm test` does all of this in a temporary folder, installs the two tarballs into a fresh
project without network access and runs `earnest verify` there.

Each platform package is assembled on a machine of that platform, from a release build made
there. The versions of `@earnest/cli` and of every `platforms/*/package.json` are the same, and
a test fails when they differ.
