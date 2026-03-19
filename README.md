# bark

**Smart file header management and directory tree generator.**

`bark` stamps standardized path headers onto source files and generates a directory tree — keeping every file self-documenting, every project consistently tagged, and your codebase navigable at a glance.

```
// File: src/main.rs
```

---

## Contents

- [Why bark](#why-bark)
- [Install](#install)
- [Quick start](#quick-start)
- [Commands](#commands)
  - [tag](#tag-default)
  - [strip](#strip)
  - [tree](#tree)
  - [watch](#watch)
  - [restore](#restore)
  - [init](#init)
- [Config file](#config-file)
- [Template system](#template-system)
- [Supported file types](#supported-file-types)
- [Backup & restore](#backup--restore)
- [Watch mode](#watch-mode)
- [Build from source](#build-from-source)
- [CI / release](#ci--release)

---

## Why bark

- **One command tags an entire project.** Run `bark` in any directory; every supported source file gets a consistent header on line 0 (or line 1 if a shebang is present).
- **Idempotent.** Running `bark` twice never duplicates or corrupts headers.
- **Non-destructive.** Before modifying any file, bark creates a timestamped backup. Every change can be reverted.
- **Template-driven.** The header text is fully configurable: embed the file path, author, date, project name, or any static variable you define.
- **Fast.** Files are processed in parallel with Rayon. Binary files, gitignored paths, and files over the size limit are skipped automatically.
- **74+ file types** across four comment styles out of the box. Extend with your own.

---

## Install

### One-liner (recommended)

Downloads the pre-built binary for your platform from the latest GitHub release.

```bash
curl -fsSL https://raw.githubusercontent.com/lordofthemind/bark/master/install.sh | bash
```

Installs to `~/.local/bin` by default. Override with `BARK_INSTALL_DIR`:

```bash
BARK_INSTALL_DIR=/usr/local/bin curl -fsSL https://raw.githubusercontent.com/lordofthemind/bark/master/install.sh | bash
```

**Supported platforms:**

| Platform | Architecture | Archive |
|---|---|---|
| Linux | x86\_64 | `bark-linux-x86_64.tar.gz` |
| Linux | ARM64 (Graviton, Raspberry Pi) | `bark-linux-aarch64.tar.gz` |
| macOS | Intel | `bark-macos-x86_64.tar.gz` |
| macOS | Apple Silicon | `bark-macos-aarch64.tar.gz` |
| Windows | x86\_64 | `bark-windows-x86_64.zip` |

### Cargo

Installs the `bark` binary to `~/.cargo/bin` (already in your `PATH` if you installed Rust via rustup):

```bash
cargo install --git https://github.com/lordofthemind/bark
```

### Build from source

```bash
git clone https://github.com/lordofthemind/bark
cd bark
cargo install --path .
```

This also installs to `~/.cargo/bin`.

---

## Quick start

```bash
# 1. Create a config file in your project
bark init

# 2. Tag all source files and generate tree.txt
bark

# 3. Preview changes without writing (dry run)
bark tag --dry-run

# 4. Remove all headers
bark strip

# 5. Generate only the directory tree, no headers
bark tree

# 6. Watch for changes and auto-tag on save
bark watch
```

---

## Commands

All commands accept `-v / --verbose` for detailed per-file output and `--config <FILE>` to point at a specific `.bark.toml`.

---

### `tag` (default)

Add or update bark-managed headers across an entire directory tree. This is the default command — running `bark` with no subcommand is equivalent to `bark tag`.

```
bark tag [OPTIONS] [DIR]
```

| Flag | Default | Description |
|---|---|---|
| `-n, --dry-run` | — | Preview what would change; write nothing |
| `-f, --force` | — | Skip backups before modifying files |
| `-o, --output <FILE>` | `tree.txt` | Output path for the directory tree |
| `-b, --backup-dir <DIR>` | `.bark_backups` | Where to store backups |
| `--template <TEMPLATE>` | — | Override header template for this run |
| `--max-size <BYTES>` | `1048576` | Skip files larger than this |
| `--threads <N>` | `0` (auto) | Rayon thread count (0 = automatic) |
| `--no-tree` | — | Skip tree.txt generation |
| `[DIR]` | `.` | Root directory to process |

**Examples:**

```bash
# Tag everything in the current directory (with tree.txt)
bark

# Tag a specific directory without creating backups
bark tag --force ~/projects/myapp

# Dry run with a one-off template
bark tag --dry-run --no-tree --template "File: {{file}} | {{author}}"

# Limit parallel threads and skip large files
bark tag --threads 4 --max-size 512000
```

**Output:**

```
bark done
  12 tagged
   3 updated
   8 current
   1 skipped
```

---

### `strip`

Remove all bark-managed headers from every file in the directory tree.

```
bark strip [OPTIONS] [DIR]
```

| Flag | Default | Description |
|---|---|---|
| `-n, --dry-run` | — | Preview which headers would be removed |
| `-b, --backup` | — | Create backups before stripping |
| `--backup-dir <DIR>` | `.bark_backups` | Backup location (used with `--backup`) |
| `[DIR]` | `.` | Root directory to process |

**Examples:**

```bash
# Strip all headers (no backups)
bark strip

# Strip with backups, preview first
bark strip --dry-run
bark strip --backup
```

---

### `tree`

Generate a directory tree file **without touching any source file headers**.

```
bark tree [OPTIONS] [DIR]
```

| Flag | Default | Description |
|---|---|---|
| `-o, --output <FILE>` | `tree.txt` | Output path for the tree |
| `[DIR]` | `.` | Root directory to scan |

**Example output (tree.txt):**

```
.
├── src/
│   ├── cli.rs
│   ├── header.rs
│   ├── lib.rs
│   ├── main.rs
│   ├── processor.rs
│   └── walker.rs
├── tests/
│   └── integration.rs
├── Cargo.toml
└── README.md
```

Hidden directories (`.git`, `.bark_backups`, etc.) and common build artifacts (`target/`, `node_modules/`, `dist/`, `build/`, `vendor/`, `__pycache__/`) are excluded automatically.

**Examples:**

```bash
# Write to the default tree.txt
bark tree

# Write to a custom file
bark tree --output docs/structure.txt

# Generate tree for a different directory
bark tree ~/projects/myapp
```

---

### `watch`

Watch a directory for file changes and automatically tag modified files as they are saved.

```
bark watch [OPTIONS] [DIR]
```

| Flag | Default | Description |
|---|---|---|
| `--debounce <MS>` | `500` | Milliseconds to wait after a change before processing |
| `-n, --dry-run` | — | Log what would be tagged without writing |
| `-o, --output <FILE>` | `tree.txt` | Output path for tree regeneration |
| `[DIR]` | `.` | Root directory to watch |

**How it works:**

1. Watches the directory recursively for create and write events.
2. Waits for the debounce window to pass (collecting burst saves).
3. Tags all changed files in parallel.
4. Regenerates `tree.txt`.
5. Skips files bark itself just wrote (prevents double-processing).

Press `Ctrl-C` to stop.

**Examples:**

```bash
# Watch the current directory
bark watch

# Watch with a shorter debounce (faster response)
bark watch --debounce 200

# Watch without writing anything (preview mode)
bark watch --dry-run
```

---

### `restore`

Restore files from bark's timestamped backups.

```
bark restore [OPTIONS]
```

| Flag | Default | Description |
|---|---|---|
| `--root <DIR>` | `.` | Project root directory |
| `--backup-dir <DIR>` | `.bark_backups` | Backup directory to read from |
| `--file <FILE>` | — | Filter to backups of a specific file |
| `-n, --dry-run` | — | Preview what would be restored |
| `--latest` | — | Auto-restore the most recent backup of each file |

**Interactive mode** (no `--latest`): lists all backups with timestamps and prompts for a number.

```
Available backups:
  [ 1] src/main.rs (2026-03-19 14:22:01)
  [ 2] src/main.rs (2026-03-18 09:10:44)
  [ 3] src/lib.rs  (2026-03-18 09:10:44)

Enter number to restore (or 0 to cancel):
```

**Examples:**

```bash
# Interactive restore
bark restore

# Auto-restore everything to its latest backup
bark restore --latest

# Restore only one file, preview first
bark restore --dry-run --file src/main.rs
bark restore --file src/main.rs

# Use a non-default backup directory
bark restore --backup-dir /path/to/backups --latest
```

---

### `init`

Write a default `.bark.toml` config file.

```
bark init [OPTIONS] [DIR]
```

| Flag | Default | Description |
|---|---|---|
| `--force` | — | Overwrite an existing `.bark.toml` |
| `[DIR]` | `.` | Directory to write the config into |

**Examples:**

```bash
# Create .bark.toml in the current directory
bark init

# Overwrite an existing config
bark init --force

# Create config in a specific directory
bark init ~/projects/myapp
```

---

## Config file

bark searches upward from the current directory for `.bark.toml`. If none is found, it checks `~/.config/bark/config.toml`. If neither exists, built-in defaults are used.

Generate a fully-commented default config with `bark init`.

```toml
[general]
output        = "tree.txt"     # tree output filename
backup_dir    = ".bark_backups" # where backups are stored
max_file_size = 1048576        # skip files larger than this (bytes, default 1 MB)
backup        = true           # create backups before modifying files

[template]
# Header text applied to every file.
# Available variables: {{file}}, {{date}}, {{year}}, {{author}},
#                      {{project}}, {{filename}}, {{ext}}
default     = "File: {{file}}"
date_format = "%Y-%m-%d"       # strftime format

# Per-extension overrides — keyed by extension without the dot
[template.overrides]
rs = "File: {{file}} | Author: {{author}} | {{date}}"
py = "File: {{file}} | Project: {{project}}"

# Static variables you can reference in any template
[template.variables]
author  = "Your Name"
project = "my-project"
team    = "backend"

[exclude]
# Glob patterns for files and directories to skip entirely
patterns = [
    "*.min.*",
    "*.bundle.*",
    "dist/**",
    "build/**",
    "node_modules/**",
    "vendor/**",
    "target/**",
]

[extensions]
# Add support for file types not in the built-in set
# style must be one of: "slash", "hash", "css", "html"
custom = [
    { ext = "lua",    style = "slash" },
    { ext = "svelte", style = "html"  },
]
# Extensions to always skip, even if they match a built-in style
skip = []

[watch]
debounce_ms = 500   # milliseconds to wait after a change before processing
ignore      = []    # additional glob patterns to ignore in watch mode
```

---

## Template system

The header text written into each file is controlled by a template string. Every supported file extension has an associated comment style; bark wraps the rendered template accordingly.

### Built-in variables

| Variable | Example value | Description |
|---|---|---|
| `{{file}}` | `src/main.rs` | Relative file path (forward slashes on all platforms) |
| `{{date}}` | `2026-03-19` | Today's date (format set by `date_format` in config) |
| `{{year}}` | `2026` | Current year |
| `{{author}}` | `Alice` | From `[template.variables] author`, then `git config user.name` |
| `{{project}}` | `bark` | From `[template.variables] project`, then parent directory name |
| `{{filename}}` | `main` | File stem (name without extension) |
| `{{ext}}` | `rs` | File extension without the dot |

Custom variables defined under `[template.variables]` are also available using the same `{{name}}` syntax.

Unknown variables are passed through unchanged.

### Comment style wrapping

The rendered template text is wrapped in the appropriate comment syntax for each file type:

| Style | Rendered header |
|---|---|
| Slash | `// File: src/main.rs` |
| Hash | `# File: scripts/build.py` |
| CSS | `/* File: styles/main.css */` |
| HTML | `<!-- File: index.html -->` |

### Shebang handling

If a file begins with a shebang (`#!/...`), the header is placed on **line 1** (after the shebang), not line 0:

```python
#!/usr/bin/env python3
# File: scripts/deploy.py

import sys
```

### Per-extension overrides

```toml
[template.overrides]
rs  = "File: {{file}} | Author: {{author}}"
sql = "File: {{file}} | Do not edit — generated"
```

Override templates apply only to files with that extension. All other files use `[template] default`.

### CLI override

Use `--template` to apply a one-off template for a single run without touching the config:

```bash
bark tag --template "File: {{file}} | {{date}}" --no-tree
```

---

## Supported file types

bark supports 74+ extensions across four comment styles.

### `//` — Slash

| Category | Extensions |
|---|---|
| C / C++ | `c` `cc` `cpp` `cxx` `h` `hpp` `hxx` |
| Java / JVM | `java` `kt` `kts` `scala` `groovy` |
| C# / .NET | `cs` `fs` `fsi` `fsx` |
| Go | `go` |
| Rust | `rs` |
| Swift / ObjC | `swift` `m` `mm` |
| JavaScript | `js` `mjs` `cjs` `jsx` |
| TypeScript | `ts` `tsx` `mts` `cts` |
| PHP | `php` |
| Dart | `dart` |
| Zig | `zig` |
| V | `v` |
| Odin | `odin` |
| Gleam | `gleam` |
| Solidity | `sol` |
| Protobuf / Thrift | `proto` `thrift` |
| Shaders | `wgsl` `glsl` `hlsl` |
| Functional | `purs` `elm` |

### `#` — Hash

| Category | Extensions |
|---|---|
| Python | `py` |
| Ruby | `rb` |
| Crystal | `cr` |
| Nim | `nim` |
| Elixir | `ex` `exs` |
| Julia | `jl` |
| Terraform / HCL | `tf` `tfvars` `hcl` |
| Nix | `nix` |
| GraphQL | `graphql` `gql` |
| Data / Config | `toml` `yaml` `yml` |
| Markdown / Text | `md` `txt` |

### `/* */` — CSS

`css` `scss` `sass` `less` `styl`

### `<!-- -->` — HTML

`html` `htm` `xml` `svg` `vue` `svelte` `astro`

### Adding custom extensions

```toml
[extensions]
custom = [
    { ext = "lua",    style = "slash" },
    { ext = "svelte", style = "html"  },
    { ext = "bicep",  style = "slash" },
]
```

### Skipping extensions

```toml
[extensions]
skip = ["md", "txt"]   # never tag these even if they have a built-in style
```

---

## Backup & restore

By default, bark creates a timestamped backup of every file it modifies. Backups live in `.bark_backups/` and mirror the source tree structure.

```
.bark_backups/
└── src/
    └── main.rs.20260319_142022.bak
```

The naming format is:

```
<relative/path/to/file>.<YYYYMMDD_HHMMSS>.bak
```

Timestamps are recorded in your local timezone, so the time shown in `bark restore` always matches your system clock.

### Disable backups

To skip backup creation for a single run:

```bash
bark tag --force
```

To disable backups permanently:

```toml
[general]
backup = false
```

### Restore latest backup

```bash
bark restore --latest
```

### Restore interactively

```bash
bark restore
```

Lists all backups with timestamps. Enter the number of the version to restore, or `0` to cancel.

### Restore a single file

```bash
bark restore --file src/main.rs
```

### Use a custom backup location

```bash
bark restore --backup-dir /mnt/safe/backups --latest
```

---

## Watch mode

`bark watch` uses the OS file-system notification API to detect file saves in real time.

```bash
bark watch
# bark  Watching /home/user/myapp for changes… (Ctrl-C to stop)
```

Every time you save a file:
1. bark waits for the debounce window (default 500 ms) to collect burst saves.
2. Modified files are tagged — the same exclude patterns, skip list, and custom extensions from your config apply.
3. `tree.txt` is regenerated.

bark tracks files it just wrote so it never enters a self-tagging loop.

Tune the debounce for faster editors or slow network filesystems:

```bash
bark watch --debounce 200   # 200 ms — snappier
bark watch --debounce 1000  # 1 s — fewer redundant writes
```

---

## Build from source

**Prerequisites:** Rust stable (1.70+)

```bash
git clone https://github.com/lordofthemind/bark
cd bark
cargo install --path .
```

This installs to `~/.cargo/bin/bark`. To only build without installing, run `cargo build --release`; the binary lands at `target/release/bark`.

**Run tests:**

```bash
cargo test
```

**Measure coverage** (requires [cargo-tarpaulin](https://github.com/xd009642/tarpaulin)):

```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html --output-dir coverage/
```

Current coverage: **92.79%** across 666 instrumented lines.

---

## CI / release

### Continuous integration

Every push to `master` and every pull request runs the full test suite on Ubuntu, macOS, and Windows:

```
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

### Publishing a release

Tag a commit to trigger the release pipeline:

```bash
git tag v1.2.0
git push origin v1.2.0
```

The release workflow:
1. Runs `cargo test` on Ubuntu.
2. Builds release binaries for all five platform/architecture targets in parallel.
3. Packages each binary as `.tar.gz` (Unix) or `.zip` (Windows).
4. Creates a GitHub Release with auto-generated release notes and attaches all binaries.

Tags containing a `-` (e.g., `v1.2.0-beta`) are automatically marked as pre-releases.

---

## License

MIT — see [LICENSE](LICENSE).

Authors: [lordofthemind](https://github.com/lordofthemind), [lordofthemind](https://github.com/lordofthemind)
