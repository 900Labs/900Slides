# Low-resource product and release requirements

Last reviewed: 2026-09-07.

900Slides is intended for older computers, intermittent connectivity, small
screens, shared machines, and unreliable power. This document turns that
intention into acceptance criteria. **The thresholds below are proposed release
targets, not measured performance or a currently supported-hardware list.**
Until a release attaches evidence, a criterion is **Not verified**. Passing a
unit test or producing a small binary does not qualify an entire device tier.

The [product specification](../PRODUCT_SPEC.md) defines scope. The
[competitive research](COMPETITIVE_ANALYSIS.md) explains the priorities.
Release notes should state which criteria actually passed.

## Device profiles

| Profile | Test configuration | Required scope |
| --- | --- | --- |
| **Primary modest computer** | 4 GB physical RAM; dual-core x86-64 CPU around 2 GHz; integrated graphics; SATA SSD; 1366×768 screen. Record exact CPU and OS/runtime versions. | Complete create/edit/save/reopen/present/export/recover workflow with the standard fixture below. |
| **Constrained computer** | 2 GB physical RAM; dual-core x86-64 CPU; integrated graphics; HDD; 1024×768 screen; lightweight desktop on a selected maintained Linux release. Record swap configuration. | Small classroom deck with images/notes; no crashes, inaccessible controls, or unrecoverable saves. Slower timing targets are explicit. |
| **Projector/shared-device check** | Primary profile plus 1024×768 second display, standard user account, and offline installation media. | Present without leaking notes, reconnect the display, transfer the file, and resume after forced process exit. |

These profiles are not demographic assumptions about every country. Validate
with teachers, students, and community users on their actual devices. A
RAM-limited virtual machine helps find regressions but does not prove HDD,
GPU, battery, or physical low-memory behavior.

## Runtime and platform boundaries

| Platform | Proposed qualification target | Evidence and support boundary |
| --- | --- | --- |
| **Windows** | Windows 10 22H2 x64 and Windows 11 x64 with a supported WebView2 runtime. | Test actual release packages. Microsoft says WebView2 on Windows 10 22H2 receives updates through at least October 2028; this does not extend the OS's own support. Windows 7/8/8.1 stopped at runtime 109 and are not targets. [Microsoft runtime/OS lifecycle](https://learn.microsoft.com/en-gb/deployedge/microsoft-edge-supported-operating-systems). |
| **Linux** | An explicit x86-64 distribution baseline with WebKitGTK 4.1. Ubuntu 22.04 and Debian 12 are candidate build baselines; qualify maintained target distributions separately. | Build on the oldest intended compatible base. A newer glibc build can fail on an older host; AppImage does not remove this constraint. Publish distribution, desktop, graphics stack, dependencies, and CPU instruction baseline. [Tauri AppImage limitations](https://v2.tauri.app/distribute/appimage/). |
| **macOS** | Named Intel and Apple Silicon configurations, qualified separately. | Tauri's development prerequisites list macOS 10.15+, not a 900Slides runtime guarantee. Test minimum deployment target, system WKWebView JavaScript/CSS behavior, signing, and installation before advertising a minimum OS. [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/), [webview architecture](https://v2.tauri.app/reference/webview-versions/). |

32-bit CPUs, unsupported operating systems, and untested architectures are not
covered by these targets. Support older hardware with a maintained OS/runtime
where possible. Framework prerequisites or a successful source build are not
equivalent to supported end-user installation.

### Offline installation

Tauri's default Windows installer downloads WebView2 when missing. Embedding a
bootstrapper still requires internet. Its offline-installer option adds an
upstream estimate of approximately 127 MB; a fixed runtime approximately
180 MB. These are not measured 900Slides artifacts. Provide a complete offline
installer, or label a smaller package as requiring an existing runtime.
[Tauri Windows installation options](https://v2.tauri.app/distribute/windows-installer/).

Qualify Linux bundles or offline dependency sets on clean machines. Document
AppImage execution/FUSE requirements; a `.deb` that needs an online package
repository is not a complete offline installer. Bundle default templates,
essential fonts, licenses, and usage instructions. End users must not need
Rust, Node.js, a package manager, an account, or activation connectivity.

## Reproducible workloads and measurement

The following are fixture specifications, not a claim that profiling has been
completed. Generate redistributable fixtures and record exact byte size and
SHA-256 with each result.

- **Small classroom deck:** 20 slides; two text boxes per slide; 10 generated
  1280×720 JPEG images across the deck; notes on every slide; no video or
  remote assets; compressed PPTX at most 10 MiB.
- **Standard deck:** 50 slides; five objects per slide; 20 generated
  1920×1080 JPEG images; one 10×5 table and one chart; notes;
  compressed PPTX at most 25 MiB.
- **Stress deck:** 200 slides and 100 generated images; separate corrupt,
  oversized, and unsupported-feature fixtures. Tests bounded failure and
  recovery, not unconstrained editing on a 2 GB computer.

### Generated smoke fixtures

A standard-library-only generator provides original, redistributable smoke decks:

```bash
python3 scripts/generate-perf-fixtures.py --output-dir /tmp/900slides-fixtures --workload all --include-negative
```

It never overwrites existing files and records sizes and SHA-256 values in a
manifest. Classroom, standard and stress decks follow the slide/object/pixel
counts above, but use highly compressible synthetic **PNG** illustrations instead
of JPEG photographs. They exercise decoding and editing paths; they do not qualify
the compressed-photo, disk-I/O or memory budgets. Negative fixtures cover an
unsupported group, a missing required part and an oversized ZIP entry. Package/XML
validation by the generator is not an application or external-office validation.

Use release builds with development tools closed. Distinguish cold and warm
storage/OS caches. Measure the native process and webview/renderer children
together. Record the actual memory metric, CPU, save-time peak, elapsed time,
and swap behavior; cross-platform RSS/private-memory values are not
interchangeable. Save anonymized results with revision and artifact checksum.

## Acceptance matrix

All numeric limits are initial engineering budgets to validate on the profiles
above. Change them through a documented product decision with measurements,
not by silently weakening a failed check.

| ID | Criterion and initial target | Required evidence | Qualification |
| --- | --- | --- | --- |
| **LR-01** | Install and first-launch from complete offline media with networking disabled and no account. All bundled layouts/fonts load. No application telemetry, analytics, remote assets, or remote calls. | Clean-machine offline create/edit/save/export/present run; inspect app-originated network attempts on a connected test machine too. Identify independent OS/runtime updater traffic separately. | Not verified end to end |
| **LR-02** | Cold launch to usable editor ≤5 s primary / ≤10 s constrained. | Five cold launches per target; median, slowest, method, runtime version. | Not verified |
| **LR-03** | Small-deck steady-state process-tree memory ≤350 MiB and peak ≤512 MiB constrained; standard-deck peak ≤768 MiB primary. No sustained swap during ordinary typing. | OS-appropriate process-tree measurement while opening, editing, undoing, saving, presenting; include webview children and name the metric. | Not verified |
| **LR-04** | Input-to-visible-text latency p95 ≤100 ms primary / ≤200 ms constrained; slide navigation p95 ≤150 ms / ≤300 ms. Save standard/small fixture ≤3 s / ≤5 s respectively. | At least 100 typing/navigation actions and five saves. Exclude human typing delay, include IPC/render work; verify saved contents. | Not verified |
| **LR-05** | After 30 s settling, idle CPU averaged for 60 s ≤1% of one logical CPU; static presentation with timer ≤3%. No continuous idle/hidden animation loop. | Process-tree CPU samples on AC and battery; note sampling precision. Verify reduced-motion behavior. | Not verified |
| **LR-06** | Essential actions/dialogs reachable at 1366×768 and 1024×768, 100% scaling. At 200% text/UI scaling, controls reflow or scroll without hidden actions. Canvas may pan/zoom independently. | Native screenshots and keyboard tasks: open/save, insert text/image, layout, reorder, notes, present, recover. Include overflow menus, modal controls, focus visibility. | Not verified across native targets |
| **LR-07** | Keep the existing ≤16 MiB release-executable budget. Publish actual download, installed, runtime, and font bytes separately. Explain complete offline dependencies for every artifact. | Fresh packaged release/checksums; `scripts/verify-wave22.sh` measures the executable, not the installation. Test removable-media setup; establish full-download budgets after first measured packages. | Executable gate exists; full artifacts unqualified here |
| **LR-08** | Forced exit after an edit settles must recover it; target ≤2 s after the last successful edit on both profiles. Failed save, full disk, permission denial, or interrupted replacement preserves the prior valid file and recovery. | Fault-injection/manual runs at edit/recovery/save boundaries; reopen and compare contents. No blanket zero-data-loss claim. | Not verified on target hardware |
| **LR-09** | Supported PPTX text remains editable. Check no-op preservation and edited-part preservation separately. Unsupported content has per-slide warnings. PDF text is selectable where supported. | Generated round trips plus opening in named PowerPoint/Impress versions; verify notes, order, images, fonts, warnings. ZIP parsing alone is insufficient. | Interoperability qualification pending |
| **LR-10** | Advertised default fonts/templates work offline. Missing fonts produce explicit substitution notices; unsupported glyphs are not silently accepted. | Save/reopen/present/export Latin diacritics, Arabic RTL, Devanagari, CJK, mixed-direction fixtures; record gaps and font licenses. | Script/font coverage pending |
| **LR-11** | Core tasks keyboard-operable; visible focus restored after dialogs; screen-reader names. Contrast ≥4.5:1 normal text / ≥3:1 large text. Targets ≥24×24 CSS px or compliant spacing. Alternatives to dragging. | Manual NVDA/Windows, Orca/Linux, VoiceOver/macOS on claimed platforms; contrast measurement and keyboard walkthroughs. | Accessibility qualification pending |
| **LR-12** | Externalizable UI strings; test 40% text expansion and RTL layout before advertising translations. State English-only UI until translated flows are reviewed. | Pseudolocale screenshots and human review per advertised language, including error/recovery text and shortcuts. | Localization qualification pending |
| **LR-13** | Large/malformed files fail without replacing the deck, unbounded allocation, or a frozen editor. Heavy operations expose progress/cancellation where meaningful. | Stress/corrupt fixtures, memory caps, cancellation/recovery checks; document import limits and supported subsets. | Stress qualification pending |
| **LR-14** | New user completes a five-slide lesson/community update with local text/images, layout, notes, save/reopen, presentation, and handover file within 20 minutes after short orientation. | At least five consenting target users; record task completion/blockers without presentation content. Pilot gate, not a population claim. | User validation pending |

LR-11 draws on [WCAG 2.2](https://www.w3.org/TR/WCAG22/) and
[target-size guidance](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html).
These checks cover part of accessibility, not certification. Canvas geometry
does not justify inaccessible surrounding menus, dialogs, or notes.

## Priority and release reporting

File safety, basic editing, keyboard use, compact-screen access, and an honest
installation path block an everyday-user claim. Measure performance, font
coverage, and representative interoperability before advertising 2 GB/4 GB
profiles as supported. Additional animation, AI, recording, and online
integrations must not displace that work.

Each release result should include revision; package checksum; OS/CPU/RAM/
storage/display/runtime; fixture hash; measured values; pass/fail per LR ID;
limitations; and reproducible steps. Link it from release notes. Keep
**implemented**, **automatically checked**, **manually verified**, and
**not verified** distinct. Recheck lifecycle/dependency sources before changing
advertised OS support.
