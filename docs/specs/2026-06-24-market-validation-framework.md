# Market validation & adoption framework

**Date:** 2026-06-24
**Status:** Active plan for the public listing. Goal: ship a near-zero-effort listing and **measure interest via GitHub stars** (plus free GitHub Insights) — no analytics service, no waitlist form — then **gate each next investment on the signal** (don't build signing, store submissions, Linux builds, or roadmap features until demand shows).

## The listing (what ships now)
- **Landing site** (`zanto-site`, GitHub Pages): value + 30-sec demo + "nothing leaves your machine" up top.
- **macOS / Windows:** **unsigned** builds, downloaded from **GitHub Releases**.
- **Mac App Store / Microsoft Store:** listed as **"coming soon"** — follow via **⭐ Star / Watch**.
- **Linux:** **self-serve** — build from source.
- **GitHub repo:** the hub *and* the dial.

## Audience — broad, no narrowing
Lead with the value; let it find its people. Privacy-conscious users across the spectrum: technical users who want a private, local AI workspace **and** the non-technical people they set it up for (a techie installing it for family removes the setup friction/risk, so "non-tech" isn't excluded — it's reached through them).

## Measurement: GitHub only (zero extra infra)
Rely on **GitHub stars** as the primary interest dial, backed by what GitHub already provides free:
- **Stars** — the headline vote.
- **Insights → Traffic** — views, unique visitors, clones, and **referrers** (shows if it's spreading beyond GitHub).
- **Issues / Discussions** — qualitative demand: real questions, requests, "I'd use this for…".

No event-tracking service, no Cloudflare Worker, no Google Form. In-app telemetry stays **off** ("no telemetry" is a selling point).

## Read the signal honestly
- The listing is friction-heavy (unsigned builds, self-build Linux) → **downloads ≠ demand**; don't over-read low download counts.
- **Stars lean vanity** (drive-by interest). Within GitHub, weight **issues/discussions with real intent** and **non-GitHub referrer traffic** higher than the raw star count — those mean someone actually wants it or shared it beyond the dev bubble.

## Gates — invest only when the signal trips (numbers are a starting bar; tune to taste)
- **Stage 0 — Listed (now).** Site + Releases downloads + public repo. Cost ≈ 0. **Measure 4–6 weeks.**
- **Gate 1 → kill the friction (code signing + notarization).** Trigger: organic interest — e.g. **≥ 50 stars** *or* **first unsolicited issues/discussions**. Highest-ROI step; prerequisite for the stores.
- **Gate 2 → distribution (Mac App Store / Microsoft Store).** Trigger: sustained star + traffic growth after signing.
- **Gate 3 → reach (prebuilt Linux: AppImage/deb/flatpak).** Trigger: explicit Linux demand — issues asking + repo clones.
- **Gate 4 → depth (roadmap features, e.g. the finance inbox).** Trigger: real **usage feedback** in issues, not vanity stars.

## Cadence & timebox
- **Weekly (2 min):** stars · Insights traffic/referrers · new issues/discussions.
- **Monthly:** review against the active gate → **invest / hold / kill**.
- **Timebox: 10 weeks** for Stage 0. If no gate trips, that's a *decision* (hold/kill), not drift.

## Cheap moves that raise signal quality
- A **30-sec demo (GIF/video)** in the README + site — top conversion lever.
- README hero + a clear **unsigned-warning install note** (recovers silent bounces); privacy promise up top.
- A frictionless **"⭐ Star on GitHub"** CTA on the site and in the README.
- **Discussions on** + an **issue template** — give qualitative signal a home.

## Deliberately NOT doing
Event-tracking/analytics service, waitlist/email capture, bank sync, cloud sync, mobile, paid acquisition. Premise: **measure organic interest cheaply via GitHub, earn each next investment.**

## To finalize
Tune the gate numbers + the 10-week timebox to your appetite. Everything else is ready.
