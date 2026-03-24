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
- [Ignoring files](#ignoring-files)
- [Backup & restore](#backup--restore)
- [Watch mode](#watch-mode)
- [Build from source](#build-from-source)
- [CI / release](#ci--release)

---

## Why bark

- **One command tags an entire project.** Run `bark` in any directory; every supported source file gets a consistent header on line 0 (or line 1 if a shebang is present).
- **Idempotent.** Running `bark` twice never duplicates or corrupts headers.
- **Non-destructive.** Before modifying any file, bark creates a timestamped backup. Every change can be reverted with `bark restore`.
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

> **First install?** Make sure `~/.local/bin` is in your `PATH`. Add this to your shell config (`~/.bashrc`, `~/.zshrc`, `~/.config/fish/config.fish`, etc.) if it isn't already:
>
> ```bash
> export PATH="$HOME/.local/bin:$PATH"
> ```

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
# 1. Create a config file in your project (optional but recommended)
bark init

# 2. Tag all source files and generate tree.txt
bark

# 3. Preview changes without writing anything
bark tag --dry-run

# 4. Watch for changes and auto-tag on every save
bark watch

# 5. Remove all headers
bark strip

# 6. Generate only the directory tree, no headers touched
bark tree
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
| `--max-size <BYTES>` | `1048576` | Skip files larger than this (default 1 MB) |
| `--threads <N>` | `0` (auto) | Rayon thread count (0 = use all cores) |
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

# Limit parallel threads and skip files over 512 KB
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
# Preview first, then strip
bark strip --dry-run
bark strip

# Strip with backups so you can restore later
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

**Example output (`tree.txt`):**

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

> **Note:** `bark tag` generates `tree.txt` automatically after tagging. Use `--no-tree` to skip it or `bark tree` to generate it standalone.

**Examples:**

```bash
bark tree

# Write to a custom path
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
2. Waits for the debounce window to pass (collecting burst saves from editors).
3. Tags all changed files in parallel — same exclude patterns, skip list, and custom extensions from your config apply.
4. Regenerates `tree.txt`.
5. Tracks files bark itself just wrote to prevent self-tagging loops.

Press `Ctrl-C` to stop.

**Examples:**

```bash
# Watch current directory
bark watch

# Snappier response for fast editors
bark watch --debounce 200

# Preview mode — see what would be tagged without writing
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
# Interactive restore — pick which backup to restore
bark restore

# Automatically restore every file to its latest backup
bark restore --latest

# Preview what --latest would restore without writing
bark restore --dry-run --latest

# Restore only one file (interactive)
bark restore --file src/main.rs

# Use a non-default backup directory
bark restore --backup-dir /mnt/safe/backups --latest
```

---

### `init`

Write a default `.bark.toml` config file in the specified directory.

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

bark searches upward from the current directory for `.bark.toml`. If none is found, it checks `~/.config/bark/config.toml`. If neither exists, built-in defaults are used — no config file is required.

Generate a fully-commented default config with:

```bash
bark init
```

### Full reference

```toml
[general]
output        = "tree.txt"      # tree output filename
backup_dir    = ".bark_backups" # where backups are stored
max_file_size = 1048576         # skip files larger than this (bytes, default 1 MB)
backup        = true            # create backups before modifying files

[template]
# Header text applied to every file.
# Available variables: {{file}}, {{date}}, {{year}}, {{author}},
#                      {{project}}, {{filename}}, {{ext}}
default     = "File: {{file}}"
date_format = "%Y-%m-%d"        # strftime format

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
# Glob patterns — bark skips any file whose relative path matches
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
    { ext = "lua",   style = "slash" },
    { ext = "bicep", style = "slash" },
]
# Extensions to always skip, even if they match a built-in style
skip = ["txt"]

[watch]
debounce_ms = 500   # milliseconds to wait after a change before processing
ignore      = []    # additional glob patterns to ignore during watch mode
```

### Config precedence

```
--config <FILE>  →  .bark.toml (upward search)  →  ~/.config/bark/config.toml  →  built-in defaults
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
| `{{author}}` | `Alice` | `[template.variables] author` → `git config user.name` → `"unknown"` |
| `{{project}}` | `bark` | `[template.variables] project` → parent directory name |
| `{{filename}}` | `main` | File stem — name without extension |
| `{{ext}}` | `rs` | File extension without the dot |

Custom variables defined under `[template.variables]` are available with the same `{{name}}` syntax. Unknown variables are passed through unchanged.

### Comment style wrapping

| Style | Example header |
|---|---|
| Slash | `// File: src/main.rs` |
| Hash | `# File: scripts/deploy.py` |
| CSS | `/* File: styles/main.css */` |
| HTML | `<!-- File: templates/index.html -->` |

### Shebang handling

If a file begins with `#!`, the header is placed on **line 1** (after the shebang), not line 0:

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

### One-off CLI override

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
| Plain Text | `txt` |

### `/* */` — CSS

`css` `scss` `sass` `less` `styl`

### `<!-- -->` — HTML

`html` `htm` `xml` `svg` `vue` `svelte` `astro`

### Adding custom extensions

```toml
[extensions]
custom = [
    { ext = "lua",   style = "slash" },
    { ext = "bicep", style = "slash" },
    { ext = "njk",   style = "html"  },
]
```

### Skipping built-in extensions

```toml
[extensions]
skip = ["txt", "toml"]   # never tag these even if they have a built-in style
```

---

## Ignoring files

bark respects multiple layers of ignore rules, evaluated in this order:

1. **`.gitignore`** — any `.gitignore` in the tree, the global git ignore (`core.excludesFile`), and `.git/info/exclude` are all honoured automatically via the `ignore` crate.
2. **`.barkignore`** — a bark-specific ignore file using the same gitignore syntax. Useful when you want bark to skip files without modifying `.gitignore`.
3. **`[exclude] patterns`** in `.bark.toml` — glob patterns applied on top of the above.
4. **`[extensions] skip`** — skip specific file extensions entirely.

### Example `.barkignore`

```gitignore
# Don't tag generated files
src/generated/
*.pb.go

# Don't tag vendored code
third_party/
```

Place `.barkignore` in your project root (same directory as `.bark.toml`).

---

## Backup & restore

By default, bark creates a timestamped backup of every file it modifies. Backups live in `.bark_backups/` and mirror the source tree structure.

```
.bark_backups/
└── src/
    ├── main.rs.20260319_142022.bak
    └── main.rs.20260318_091044.bak
└── lib.rs.20260318_091044.bak
```

The naming format is:

```
<relative/path/to/file>.<YYYYMMDD_HHMMSS>.bak
```

Timestamps are recorded in your local timezone.

### Disable backups for one run

```bash
bark tag --force
```

### Disable backups permanently

```toml
[general]
backup = false
```

### Restore the latest backup of every file

```bash
bark restore --latest
```

### Restore interactively

```bash
bark restore
```

Lists all backups with timestamps. Enter the number to restore, or `0` to cancel.

### Restore a single file

```bash
# Preview first
bark restore --dry-run --file src/main.rs

# Then restore
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

Tune the debounce:

```bash
bark watch --debounce 200   # 200 ms — snappier for fast editors
bark watch --debounce 1000  # 1 s — fewer writes on slow network filesystems
```

---

## Build from source

**Prerequisites:** Rust stable (1.70+)

```bash
git clone https://github.com/lordofthemind/bark
cd bark
cargo install --path .
```

Installs to `~/.cargo/bin/bark`. To only build without installing:

```bash
cargo build --release
# binary at: target/release/bark
```

**Run tests:**

```bash
cargo test
```

**Run the full CI check locally** (same as what CI runs — catches lint and format issues before pushing):

```bash
cargo test && cargo clippy -- -D warnings && cargo fmt --check
```

**Measure coverage** (requires [cargo-tarpaulin](https://github.com/xd009642/tarpaulin)):

```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html --output-dir coverage/
```

Current coverage: **91.08%** across 706 instrumented lines.

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

Bump the version in `Cargo.toml`, commit, then tag:

```bash
# Edit Cargo.toml: version = "x.y.z"
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to vX.Y.Z"
git push

git tag vX.Y.Z
git push origin vX.Y.Z
```

The release workflow:
1. Runs `cargo test` on Ubuntu.
2. Builds release binaries for all five platform/architecture targets in parallel.
3. Packages each binary as `.tar.gz` (Unix) or `.zip` (Windows).
4. Creates a GitHub Release with auto-generated release notes and attaches all binaries.

Tags containing `-` (e.g., `v1.0.0-beta`) are automatically marked as pre-releases.

---

## License

MIT — see [LICENSE](LICENSE).
