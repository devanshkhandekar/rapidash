# Publishing Rapidash to GitHub

This guide is for the existing `sysguard-rs` Rust/GTK4 project. Commands assume Ubuntu/Debian and Bash.

## A. Prepare the working tree

```bash
cd /absolute/path/to/sysguard-rs
pwd
test -f Cargo.toml && test -f src/main.rs && echo 'Rust project found'
```

Extract the downloadable starter zip (update its actual path if different) and **preserve your current README**:

```bash
unzip -q ~/Downloads/rapidash-open-source-starter.zip -d /tmp
if [ -f README.md ]; then cp README.md /tmp/rapidash-README.before-open-source.md; fi
if [ -f .gitignore ]; then cp .gitignore /tmp/rapidash-gitignore.before-open-source; fi
cp -an /tmp/rapidash-open-source-starter/. .
# Adopt the new GitHub landing page; its prior version is backed up above.
cp /tmp/rapidash-open-source-starter/README.md README.md
```

If `.gitignore` existed, **merge it manually** with the starter version (the old file is saved in `/tmp`). If you already have a `LICENSE` file, inspect it instead of assuming the new MIT file replaced it. Review the README against your running app: this package was prepared from project discussions, **not from a full inspection of your local source tree**. Keep obsolete development notes outside the main README unless they remain useful and accurate.

## B. Inspect for sensitive or unlicensed content

Do not publish `.env`, tokens, signing keys, credential JSON, personal files, downloaded data, unlicensed photos/icons, copyrighted game artwork, or large build outputs. Follow [third-party license review](THIRD_PARTY.md) for dependencies and embedded assets. If you have an existing Git repository, review historical commits too.

```bash
# Review suspicious paths (false positives are expected).
find . -type f \( -name '.env*' -o -name '*.pem' -o -name '*.key' -o -iname '*secret*' \) \
  -not -path './target/*' -print

# Inspect code and assets, and verify ownership/license of copied content.
git status --short 2>/dev/null || true
```

For a repository with existing Git history, removing a secret from the current working tree is insufficient: old commits may still contain it. Rotate exposed credentials and clean history *before* publishing.

## C. Validate the application locally

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libgtk-4-dev libadwaita-1-dev
cargo check
cargo test
cargo run --release
```

Optional formatting check:

```bash
cargo fmt --all -- --check
```

Update `Cargo.toml` under `[package]` to include `license = "MIT"` and a description, **only if these keys do not already exist**. For example:

```toml
# Add inside the existing [package] section (do not add a second section).
description = "Linux system monitor and memory management utility built with Rust and GTK4"
license = "MIT"
# Add this after the GitHub repository exists:
# repository = "https://github.com/YOUR_GITHUB_USERNAME/rapidash"
```

Keep `Cargo.lock` tracked for this application. Ensure GitHub CI passes or explain any known limitations; do not state that builds are reproducible until tested.

## D. Initialize and stage Git

```bash
# This preserves an existing Git repository and history.
if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then git init -b main; fi

git status --short
# Confirm nothing private was hidden by a global ignore or tracked previously.
git ls-files | grep -E "(^|/)(target|\.env|credentials|secrets)(/|\.|$)" || true
git add .
git diff --cached --name-only
git diff --cached --check
```

Review `git diff --cached` and confirm that nothing sensitive is staged **before committing**.

```bash
git commit -m "docs: prepare Rapidash for open-source development"
git branch -M main
```

If Git asks for an identity, set `git config --global user.name "YOUR NAME"` and `git config --global user.email "YOUR VERIFIED GITHUB EMAIL"` and retry.

## E. Publish with GitHub CLI (recommended)

```bash
sudo apt install -y gh
gh auth login
# Only if there is no existing origin remote:
gh repo create rapidash \
  --public \
  --source=. \
  --remote=origin \
  --push \
  --description "Open-source Linux system monitoring and memory management utility built with Rust and GTK4"
```

If the GitHub repository exists already, or `origin` already points to another location, do **not** create a duplicate; check `git remote -v` and either push to the correct remote or explicitly configure it.

## E2. Git-only alternative

In GitHub's web interface, create an **empty public** repo named `rapidash`. Do **not** initialize it with README, license, or gitignore. Then:

```bash
git remote add origin https://github.com/YOUR_GITHUB_USERNAME/rapidash.git
git push -u origin main
```

If `origin` exists, inspect it with `git remote -v` rather than repeating `git remote add origin`.

## F. Post-launch

1. Verify README, license detection, and repo visibility.
2. Open Actions and inspect the first CI result. Do not display a passing badge until it passes.
3. In Settings enable Issues and Discussions; configure Topics: `rust`, `linux`, `gtk4`, `libadwaita`, `system-monitor`, `memory-management`, `open-source`.
4. In Settings → Security & analysis, enable relevant dependency security checks and, if available, Private Vulnerability Reporting.
5. Add real redacted UI screenshots and original branding per `docs/ASSETS.md`.
6. Update `SECURITY.md` and `CODE_OF_CONDUCT.md` with a confidential contact channel before actively soliciting contributions.
7. Tag a release only after a reproducible build and user-facing behavior are verified. Use the [release checklist](RELEASE_CHECKLIST.md).
8. Update your GitHub repository About text and attach an original social-preview image when ready.

If `gh repo create` fails because `origin` already exists or a repository with the name already exists, **do not delete anything automatically**. Run `git remote -v` and `gh repo view`, then select the correct remote/repository.
