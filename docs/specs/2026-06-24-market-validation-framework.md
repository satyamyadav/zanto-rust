# Market validation & adoption framework

**Date:** 2026-06-24
**Status:** Active plan for the public listing. Goal: ship a near-zero-effort listing, measure *real* interest with a free/privacy-friendly stack, and **gate each next investment on a signal** — don't build signing, store submissions, Linux builds, or roadmap features until demand shows.

## The listing (what ships now)
- **Landing site:** value + 30-sec demo + "nothing leaves your machine" front and center.
- **macOS / Windows:** **unsigned** builds, direct download from the site.
- **Mac App Store / Microsoft Store:** listed as **"coming soon"** with a *Notify me* action.
- **Linux:** **self-serve only** — clone the repo and build; no prebuilt binaries.
- **GitHub repo:** the engagement hub (stars / issues / discussions / forks).

## ⚠️ Read the numbers correctly — the funnel distorts the signal
This listing is **friction-heavy**, which suppresses the very interest it measures:
- Unsigned Mac/Win builds trigger Gatekeeper / SmartScreen scare screens → most non-technical users bounce at download. **Download count ≠ demand** (it's demand × willingness-to-bypass-a-warning).
- Linux self-build converts only developers.
- ⇒ **Do not read low downloads as "no interest."** Weight the low-friction intent signals (waitlist clicks, GitHub stars, issues/comments) — they measure *want* without forcing the user past the scary binary.

**Truth ranking:** waitlist signups + issues-from-real-users ≫ download counts ≫ stars (vanity). Weight the first ~5–10×.

## Free measurement stack
| Job | Tool | Notes |
|---|---|---|
| Visitors **vs** clicks (funnel) | **GoatCounter** (hosted) — or **Umami** self-hosted if commercial-from-day-one | privacy-first, no cookies/banner; fire custom events on each button |
| Waitlist + optional email + 1–2 qualifying Qs | **Google Form** → Google Sheets | free, zero infra; export + email at launch; keep email **optional** |
| No-email vote | **GitHub star** | true single click, suits the dev audience |

**Event wiring (analytics):** fire a custom event on each CTA — `download-mac`, `download-win`, `linux-build`, `notify-click`. The dashboard then shows visits → each click = the conversion funnel.

**Waitlist UX (single click, email optional):** the *Notify me* button (a) fires `notify-click` (counts interest even with no email), then (b) opens the Google Form (optional email + "which platform?" / "what would you use it for?"). So a click always counts; the form captures emails from those who follow through; and the Sheet is who you email when the store build lands.

**Telemetry:** in-app telemetry stays **off** (and say so — "no telemetry" is a selling point). All measurement is site-side + GitHub, which is public/aggregate.

## The gates — invest only when the trigger is met
Numbers below are a **starting bar — tune to your appetite.** The principle matters more than the exact figure: weight *intent/issues* over stars.

- **Stage 0 — Listed (now).** Site + unsigned Mac/Win downloads + Notify-me form + public repo + analytics. Cost ≈ 0. **Measure 4–6 weeks.**
- **Gate 1 → kill the friction (code signing + notarization).** Trigger: organic interest — e.g. **≥ 50 GitHub stars** *or* **≥ 30 waitlist signups** *or* **≥ 3 unsolicited issues/comments** within the window. Highest-ROI step: turns scary downloads into trusted ones *and* is the prerequisite for the stores.
- **Gate 2 → distribution (Mac App Store / Microsoft Store).** Trigger: signed-build download clicks trending up **and** sustained waitlist growth (e.g. **≥ 100 signups** or steady weekly adds). Convert the waitlist on launch.
- **Gate 3 → reach (prebuilt Linux: AppImage/deb/flatpak).** Trigger: explicit Linux demand — issues asking for it and/or repo clones (e.g. **≥ 5 Linux requests**). Mobile/sync only if separately demanded.
- **Gate 4 → depth (roadmap features, e.g. the finance inbox).** Trigger: **usage feedback** from real users (issues/emails describing real use), not vanity stars.

## Cadence & timebox
- **Weekly (5 min):** glance at the dashboard — visits · download clicks · waitlist signups · stars · new issues.
- **Monthly:** review against the *active* gate → decide **invest / hold / kill**.
- **Timebox: 10 weeks** for the Stage-0 experiment. If no gate trips by then, that's a *decision* (hold or kill), not drift.

## Cheap moves now that raise signal *quality* (so a null result is trustworthy)
- A 30-sec value demo (GIF/video) on the site — top conversion lever.
- A short note explaining the unsigned-warning + how to proceed (recovers silent bounces); privacy promise up top.
- Frictionless **"⭐ Star on GitHub"** + **"Notify me"** CTAs.
- Issue templates + GitHub Discussions on — give qualitative signal a home.

## What this framework deliberately does NOT do
Bank/Plaid sync, cloud sync, mobile, paid acquisition. The premise is *measure organic interest cheaply, then earn each next investment.*

## To finalize
Adjust the **placeholder gate numbers** and the **10-week timebox** to your appetite; everything else (the free stack, event wiring, waitlist UX, truth ranking) is ready to execute.
