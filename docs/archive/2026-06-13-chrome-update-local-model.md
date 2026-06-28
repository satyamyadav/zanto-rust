# Proof Artifact — zanto updates Chrome on Arch (local model, separate machine)

**Date:** 13 June 2026
**Status:** Queued proof artifact for article + AI Tinkerers talk. Do not lose.

---

## One-line summary

A 14B local model running on a MacBook Air drove zanto (on a separate Arch box)
through a complete, real-world system task — updating Google Chrome via a git AUR
package — with a human-in-the-loop permission gate on every mutation.

## Architecture (the diagram that carries the whole thesis)

```
Mac (M3 Air, 16GB)                 Arch box (Ryzen 5 5600G)
┌─────────────────────┐            ┌──────────────────────────────┐
│ Ollama              │            │ zanto core (Rust)            │
│ qwen2.5:14b Q4_K_M  │ ⇄ network ⇄│  - tool registry (MCP)       │
│ ctx 4096            │            │  - Approver gate (a/s/f/d)   │
│ ~9.3GB VRAM         │            │  - executor (concurrent read │
└─────────────────────┘            │    / sequential write)       │
                                   └──────────────┬───────────────┘
                                                  │
                                          system: git, makepkg, pacman
```

The inference node and the machine being acted on are DIFFERENT machines.
The agent's brain is interchangeable; the governed execution layer is what you trust.

## Model details

- qwen2.5:14b, Q4_K_M, 14.8B params, gguf, family qwen2
- ~9.3GB in VRAM, context_length 4096
- Running via Ollama on the Mac, not the Arch box

## What happened (the interesting parts, honestly)

1. First instinct (`google-chrome --version`) failed — command not found. Model
   defaulted to generic apt/dnf advice (wrong for Arch). Needed user steering.
2. User pointed it at `/home/lazy/Apps/google-chrome`. Model inspected the dir,
   correctly identified it as an Arch PKGBUILD/AUR-style repo.
3. User confirmed "git repo, system is arch."
4. Model tried `git pull origin main` → failed (no main branch).
5. **Recovered on its own**: ran `git ls-remote --heads origin`, found `master`,
   pulled master successfully. (This self-recovery is the highlight.)
6. Ran `makepkg --noconfirm -si`, built google-chrome 149.0.7827.114-1, installed
   via pacman, post-install hooks ran. Success.

## The honest limits (this is the credible part — keep it)

- Needed domain hints: that it was git, that the system was Arch, the repo path.
- Not autonomous. Hand-holding required.
- Context window 4096 is tight for agentic loops — context-trimming policy matters.
- A 14B local model is capable of safe real-system agency WITH scaffolding, not without.

## Engineering details worth featuring

- `warn: model returned unparsed tool call(s), applying fallback parser` — the
  system recovered from malformed tool calls instead of crashing. Real-platform detail.
- Permission gate fired on every write (`read`/`write` resolved paths, a/s/f/d prompt).
- Concurrent read / sequential write tool execution ordering.

## Thesis payoff

Small model + tight tool contracts + human-in-loop guard + interchangeable inference
node = safe local agency. The model wires data and picks tools; it never freelances
on anything that mutates the system without a gate.

This is the local-first instantiation of deterministic generative UI / governed agency.

## Forward look (for the article's "what's next")

- zanto → desktop app (Tauri)
- finance = first feature module
- Arch-box GPU upgrade (4060 Ti 16GB + RAM) justified not by "can't run models" but
  by: bigger context windows + inference and execution on one machine for the desktop app.

## Assets to capture

- [ ] Raw terminal transcript (preserved below as appendix; keep the original too)
- [ ] 60-second screen recording of the session for the talk
- [ ] Redraw the architecture diagram cleanly for article + zanto README + talk slide

## Candidate article title

"I let a 14B model on a MacBook Air update Chrome on my Arch box. Here's what the
architecture had to get right."

---

## Addendum — same task, cloud model (Gemini Flash)

**Date:** 13 June 2026
**Model:** `gemini-flash-latest` (Google Gemini API, live), same zanto core, same Arch box.
Switched in with a ~6-line routing change: model name starting with `gemini` skips
the Ollama endpoint override and lets genai resolve Gemini's endpoint + `GEMINI_API_KEY`.

### The comparison that carries the article

| | qwen2.5:14b (local, Mac) | gemini-flash-latest (cloud) |
|---|---|---|
| Found the repo path | needed user to supply it | discovered it itself |
| Knew it was git / Arch | needed user to say so | inferred from `git status`, `pacman` |
| Update decision | blindly ran `makepkg` | compared installed version + `HEAD..origin/master`, concluded already-current |
| Hand-holding | heavy | none |
| Failure recovery | found `master` after `main` failed | same class of recovery, unprompted |

Same scaffolding, same guard, same tools — the model's capability is the only variable.
That **is** the thesis: the brain is swappable, the governed execution layer is constant.

### Emergent behaviour worth featuring

At TURN 4 Gemini read this very proof-artifact doc
(`docs/stories/2026-06-13-chrome-update-local-model.md`) to learn where Chrome lives,
then used that to locate `/home/lazy/Apps/google-chrome`. The success story we parked
became context the next model used to solve the task. The system accretes knowledge in
the workspace, and the agent reads it back. (Local-first memory, for free.)

### What it correctly concluded

```
* Installed version: 149.0.7827.114 (google-chrome-stable --version)
* Local AUR repo at latest commit 4c16de0 on master, HEAD..origin/master empty
→ "Chrome is already up to date. No updates available."
```

It also worked out the binary is `google-chrome-stable`, not `google-chrome` (TURN 20→21).

### Friction this run exposed (honest, and now filed as bugs)

- **~10 unnecessary write prompts.** `run_command` is classified always-mutating, so
  read-only queries (`git status`, `git log`, `pacman -Qi`, `whoami`, `git remote -v`)
  all asked for *write* permission. Filed P2 in `known_issues.md`.
- **Approver brittle to typos.** TURN 16: input `a\` (stray backslash) was treated as
  deny → "permission denied", costing a turn. Anything not exactly `a`/`s`/`f` denies.

---

## Appendix — raw session transcript

Model served from Ollama on the Mac:

```json
{
  "models": [
    {
      "name": "qwen2.5:14b",
      "model": "qwen2.5:14b",
      "size": 9311758336,
      "digest": "7cdf5a0187d5c58cc5d369b255592f7841d1c4696d45a8c8a9489440385b22f6",
      "details": {
        "parent_model": "",
        "format": "gguf",
        "family": "qwen2",
        "families": ["qwen2"],
        "parameter_size": "14.8B",
        "quantization_level": "Q4_K_M"
      },
      "expires_at": "2026-06-13T00:12:22.358089+04:00",
      "size_vram": 9311758336,
      "context_length": 4096
    }
  ]
}
```

```
> update my chrome
--- TURN 1 ---
[TOOL CALL mutating] run_command (Object {"command": String("google-chrome --version")})
[TOOL OUTPUT] exit 127
[stderr]
sh: line 1: google-chrome: command not found
--- TURN 2 ---

It appears that the `google-chrome` command is not available in your environment, which suggests that Chrome might not be installed or its path is not set up correctly.

To update Google Chrome, you typically need to download the latest version from the official website or use a package manager if you are on Linux. Here's how you can check for updates and install the latest version of Chrome:

### For Debian-based Systems (like Ubuntu):
sudo apt update
sudo apt install --only-upgrade google-chrome-stable

### For Red Hat-based Systems (like Fedora or CentOS):
sudo dnf check-update google-chrome-stable
sudo dnf upgrade google-chrome-stable

Would you like to proceed with one of these methods? If so, please specify which method or distribution you are using.

> look at my ~/App directory
--- TURN 1 ---
[TOOL BATCH 1 read-only, concurrent]

[zanto] permission required: read "~/App"
  resolved: /home/lazy/App
  (a) allow once  (s) allow session  (f) allow forever  (d) deny
> a
[TOOL OUTPUT] list_directory → error: No such file or directory (os error 2)
--- TURN 2 ---

It seems that the `~/App` directory does not exist. Would you like to check another directory, or do you want to create the `~/App` directory and proceed with adding files there? Please let me know what you would like to do next.

> /home/lazy/Apps/google-chrome
--- TURN 1 ---
[zanto] warn: model returned unparsed tool call(s), applying fallback parser
[TOOL BATCH 1 read-only, concurrent]

[zanto] permission required: read "/home/lazy/Apps/google-chrome"
  resolved: /home/lazy/Apps/google-chrome
  (a) allow once  (s) allow session  (f) allow forever  (d) deny
> a
[TOOL OUTPUT] list_directory → PKGBUILD
google-chrome-stable.sh
.git/
eula_text.html
src/
pkg/
.nvchecker.toml
.SRCINFO
google-chrome-stable_143.0.7499
--- TURN 2 ---

The directory `/home/lazy/Apps/google-chrome` contains the following files and directories:

PKGBUILD
google-chrome-stable.sh
.git/
eula_text.html
src/
pkg/
.nvchecker.toml
.SRCINFO
google-chrome-stable_143.0.7499.169-1_amd64.deb
google-chrome.install
google-chrome-143.0.7499.169-1-x86_64.pkg.tar.zst

This directory appears to contain various files related to the installation and management of Google Chrome, including:

- `PKGBUILD`: A package build script for Arch Linux.
- `.git/`: A hidden directory indicating that this might be a Git repository.
- `eula_text.html`: The End User License Agreement in HTML format.
- `src/` and `pkg/`: Directories likely containing source files and packaged files, respectively.
- `google-chrome-stable.sh`: A shell script for installing Google Chrome.
- `.nvchecker.toml`: A configuration file for package version checking.

Would you like to explore any specific file or directory further?

> update google chrome
--- TURN 1 ---

To update Google Chrome, we typically don't manually look at the `/home/lazy/Apps/google-chrome` directory because Chrome updates itself automatically via its built-in updater when installed from a package manager or the official website installer.

[...generic apt/yum advice...]

Would you like to proceed with updating Chrome via the package manager?

> /home/lazy/Apps/google-chrome is git repo, system is arch
--- TURN 1 ---
[TOOL CALL mutating] run_command (Object {"command": String("cd /home/lazy/Apps/google-chrome && git pull origin main"), "working_dir": String("/home/lazy/Apps/google-chrome")})

[zanto] permission required: write "/home/lazy/Apps/google-chrome"
  resolved: /home/lazy/Apps/google-chrome
  (a) allow once  (s) allow session  (f) allow forever  (d) deny
> a
[TOOL OUTPUT] exit 1
[stderr]
fatal: couldn't find remote ref main
--- TURN 2 ---
[TOOL CALL mutating] run_command (Object {"command": String("cd /home/lazy/Apps/google-chrome && git ls-remote --heads origin")})
[TOOL OUTPUT] exit 0
4c16de07098fc757715cd57f6356114e95225445	refs/heads/master
--- TURN 3 ---
[TOOL CALL mutating] run_command (Object {"command": String("cd /home/lazy/Apps/google-chrome && git pull origin master"), "working_dir": String("/home/lazy/Apps/google-chrome")})

[zanto] permission required: write "/home/lazy/Apps/google-chrome"
  resolved: /home/lazy/Apps/google-chrome
  (a) allow once  (s) allow session  (f) allow forever  (d) deny
> a
[TOOL OUTPUT] exit 0
Updating 8cc4264..4c16de0
Fast-forward
 .SRCINFO | 6 +++---
 PKGBUILD | 4 ++--
 2 files changed, 5 insertions(+),
--- TURN 4 ---

The `git pull` command was successful, and the repository has been updated to the latest commit on the `master` branch.

Would you like to proceed with building and installing the updated package?

> y
--- TURN 1 ---

To build and install the updated Google Chrome package from the repository:

1. Build the Package: run `makepkg`.
2. Install the Built Package: `pacman -U`.

cd /home/lazy/Apps/google-chrome && makepkg --noconfirm -si

Would you like me to run this command for you?

> y
--- TURN 1 ---
[TOOL CALL mutating] run_command (Object {"command": String("cd /home/lazy/Apps/google-chrome && makepkg --noconfirm -si"), "working_dir": String("/home/lazy/Apps/google-chrome")})

[zanto] permission required: write "/home/lazy/Apps/google-chrome"
  resolved: /home/lazy/Apps/google-chrome
  (a) allow once  (s) allow session  (f) allow forever  (d) deny
> a
[sudo] password for lazy:
[TOOL OUTPUT] exit 0
==> Making package: google-chrome 149.0.7827.114-1 (Sat 13 Jun 2026 12:06:12 AM +04)
==> Checking runtime depende
--- TURN 2 ---

The update process for Google Chrome was successful. Here's a summary of what happened:

- The package `google-chrome` version `149.0.7827.114-1` was built and installed.
- Dependencies were checked, sources downloaded, and the package extracted and tidied.
- A new package was created and then installed using `pacman -U`.
- Post-installation hooks were run to update icon theme caches and desktop file MIME type cache.

Your Google Chrome installation has been updated to the latest version.
```
