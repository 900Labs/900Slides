# Presentation software: competitive research

Reviewed: 2026-09-07. Scope: presentation authoring for people using older
computers, unreliable connections, shared devices, and limited budgets.

900Slides should compete on a dependable complete workflow: install, create a
readable deck, save it safely, present it, and hand over a usable file. Offline
editing alone is not unique. Established desktop products offer it, and several
cloud products offer a prepared offline mode. Installation, file safety,
resource use, and good default layouts deserve priority over feature count.

This is a review of primary vendor/project documentation, not a hands-on
benchmark. Vendor minimum-hardware and compatibility statements are not evidence
of usable-deck performance or 900Slides fidelity. Sources were consulted on the
review date. Exact prices are omitted because editions, regions, and plans
vary. Product implications below are recommendations, not implemented-feature
claims. See the [product specification](../PRODUCT_SPEC.md),
[roadmap](ROADMAP.md), and [low-resource requirements](LOW_RESOURCE_REQUIREMENTS.md).

## Desktop and mainstream alternatives

| Product | Verified access and behavior | Product implication |
| --- | --- | --- |
| **PowerPoint desktop** | Commercial: Microsoft 365 subscriptions, one-time Office 2024, and standalone purchases exist. Installed, activated apps edit local files offline. Microsoft 365 and retail Office 2024 require periodic activation connectivity; LTSC serves disconnected organizational deployments. [Purchase/offline comparison](https://support.microsoft.com/en-us/office/what-s-the-difference-between-microsoft-365-and-office-2024-ed447ebf-6060-46f9-9e90-a239bd27eb96), [Office/LTSC FAQ](https://support.microsoft.com/en-us/office/lifecycle/office-2024-and-office-ltsc-2024-faq). | Familiar formatting, slide navigation, positioning, and notes are baseline expectations. No activation/account is a meaningful difference; do not describe all PowerPoint use as subscription-only or continuously online. |
| **PowerPoint for the web** | Free web access is available with a Microsoft account. Microsoft documents feature differences by platform; desktop Presenter View is not listed for the web version. [Free web access](https://support.microsoft.com/en-us/office/what-s-the-difference-between-microsoft-365-and-office-2024-ed447ebf-6060-46f9-9e90-a239bd27eb96), [platform comparison](https://support.microsoft.com/en-us/powerpoint/compare-powerpoint-features-on-different-platforms). | Compare the installed desktop and web offerings separately. State exactly which local editor/presenter functions 900Slides implements. |
| **Google Slides** | Account-based browser editor supports offline creation, viewing, and editing after online setup in Chrome or Edge with Google Docs Offline. Private browsing is excluded; files need local storage. [Offline instructions](https://support.google.com/docs/answer/6388102?hl=en-gb). | First use on a disconnected computer differs from prepared offline access. All local decks and bundled templates should remain available without advance selection. |
| **Apple Keynote** | Opens local Mac files as well as cloud files; some theme assets download on demand. Opens PowerPoint and exports PPTX/PDF. Its current Mac App Store listing requires macOS 15.6+ and includes in-app purchases. [Local files/themes](https://support.apple.com/guide/keynote/open-or-close-a-presentation-tan72232b56/mac), [exports](https://support.apple.com/guide/keynote/export-to-powerpoint-or-another-file-format-tana0d19882a/mac), [current listing](https://apps.apple.com/fi/app/keynote-design-presentations/id361285480?platform=mac). | Prioritize typography, spacing, and locally available layouts. A polished modern Mac experience does not represent the intended hardware population. |
| **LibreOffice Impress** | Free open-source desktop suite, MPL-2.0, with established local file workflows and community localization. Supports presentation formats including PPTX. [LibreOffice](https://www.libreoffice.org/), [format support](https://help.libreoffice.org/latest/en-US/text/shared/guide/import_ms.html). | A direct alternative for the target audience. Compare task completion and generated PPTX fixtures. Do not publish untested blanket lists of competitor fidelity failures. |
| **ONLYOFFICE Desktop Editors** | Free AGPL-3.0 suite for Windows, Linux, and macOS; explicitly supports local creation/editing without internet. Presenter mode and localization/RTL support are documented. Cloud connections are additional capabilities. [Project/license](https://github.com/ONLYOFFICE/DesktopEditors), [desktop product](https://www.onlyoffice.com/desktop). | Familiar UI, OOXML support, and offline use already coexist. 900Slides needs demonstrably simple operation and resource discipline; vendor full-compatibility claims still require fixture testing. |
| **WPS Presentation** | Proprietary suite with a free desktop offering; documents offline slideshow, editing, and PDF export alongside online products. [Offline presentation offering](https://www.wps.com/feature/the-free-online-ppt-viewer/), [requirements](https://help.wps.com/articles/system-requirements-for-wps-office/). | Free core workflows and predictable local operation matter. Verify edition entitlements and old-OS listings rather than assuming proprietary means online-only. |

## Design-led and developer alternatives

| Product | Verified behavior | Useful lesson and boundary |
| --- | --- | --- |
| **Pitch** | Supports preloaded offline editing and presenting; offline limitations include PDF/PPTX export, speaker view, and version history. [Offline guide](https://help.pitch.com/en/articles/5671537-work-offline-in-pitch). | Finish local export and recovery before expanding review or analytics features. Do not call Pitch online-only. |
| **Canva Presentations** | Supports offline editing of designs prepared online. New designs and exports require internet. Its setup page says up to 14 days offline while its storage page says 7: the duration is unresolved in vendor documentation. Offline presenting is also documented. [Offline setup](https://www.canva.com/help/set-up-offline-access/), [storage limits](https://www.canva.com/help/manage-offline-designs/), [presenting announcement](https://www.canva.com/en_in/newsroom/news/whats-new-february/). | Reusable layouts help users produce coherent work. Distinguish prepared offline editing/presenting from unrestricted local creation and export. |
| **reveal.js** | Basic HTML package runs in a browser without a build toolchain. Speaker view requires a local server. Auto-Animate matches objects across adjacent slides. [Installation](https://revealjs.com/installation/), [speaker view](https://revealjs.com/speaker-view/), [Auto-Animate](https://revealjs.com/auto-animate/). | Self-contained presentation assets and keyboard delivery are useful. HTML authoring serves a different audience; remote fonts/plugins can defeat offline packaging. |
| **Slidev** | Markdown/Vue authoring uses a local Node.js toolchain. PDF/PNG/PPTX export uses a browser through Playwright; PPTX slides are images, with notes carried separately. [Getting started](https://sli.dev/guide/), [exporting](https://sli.dev/guide/exporting). | Technical-talk features do not establish low-resource GUI suitability. Image-only PPTX is not editable interoperability. |
| **Marp** | Markdown converter with HTML/PDF/PPTX output. Ordinary PPTX export uses rendered backgrounds. Experimental editable PPTX needs both a browser and LibreOffice and documents fidelity limitations. [Marp CLI](https://github.com/marp-team/marp-cli). | Content-first templates can simplify authoring. Count the entire export toolchain in installation comparisons and label rendered versus editable output. |

## Published requirements are not benchmarks

These values are vendor requirements, not RAM consumed by an open deck.
Disk-space requirements are not download sizes. No equal-workload performance
comparison was run for this research.

| Product/source | Published requirement relevant to older computers | Interpretation |
| --- | --- | --- |
| [Office 2024 suites](https://support.microsoft.com/en-US/office/system-requirements/office-suites-for-individuals-and-families) | Windows: 4 GB RAM, 4 GB disk, 1280×768 display; current table lists Windows 11. | An older Office version running successfully does not establish current-version support. |
| [LibreOffice](https://www.libreoffice.org/system-requirements/) | Windows/Linux: 256 MB RAM, 512 MB recommended; 1280×800. TDF Linux builds list x86-64-v2 CPU, kernel 4.18+, and glibc 2.27+. | Low published RAM is not a usable-deck benchmark. CPU instructions and package variants can exclude old hardware independently of RAM. |
| [ONLYOFFICE Windows](https://helpcenter.onlyoffice.com/desktop/installation/desktop-sys-reqs-windows.aspx) | Dual-core 2 GHz CPU, 2 GB RAM, 2 GB free disk; explicitly applies to the simplest files. | Include 2 GB and 4 GB test tiers without implying complex decks run well in 2 GB. |
| [WPS](https://help.wps.com/articles/system-requirements-for-wps-office/) | Windows/Linux list 2 GB RAM; disk guidance is 4 GB on Windows and 2 GB on Linux. Older Windows versions remain listed. | Confirm exact downloadable version and maintenance status before recommending an old OS. |
| [Keynote](https://apps.apple.com/fi/app/keynote-design-presentations/id361285480?platform=mac) | Current Mac compatibility: macOS 15.6+. | Historical versions and the current downloadable app are different support cases. |

For 900Slides, publish separate measurements for the executable, installed app,
download archive, and complete offline installation including runtimes/fonts.
A shared system webview can reduce the application payload; that architecture
does not establish a RAM advantage or make an absent runtime free. The
[low-resource requirements](LOW_RESOURCE_REQUIREMENTS.md) distinguish targets
from measured results.

## Product priorities

1. **Trust the file workflow.** New/open/save/save-as, cancellation, failure
   messages, undo, and power-loss recovery must work together. Preserve
   supported content and explain unsupported content before destructive actions.
2. **Make everyday decks look intentional.** Offer a small set of local
   layouts with disciplined typography, margins, contrast, and image placement.
   Make text, images, alignment, duplication, ordering, and notes easy to find.
3. **Fit small screens.** Essential actions and dialogs must stay reachable
   at 1366×768 and 1024×768. Use collapsible panels/overflow controls instead
   of shrinking text and pointer targets until they are unreadable.
4. **Deliver and hand over reliably.** Test keyboard presentation, notes
   privacy, projector disconnects, readable PDFs, and editable PPTX. Warn about
   actual limitations rather than claiming untested fidelity.
5. **Measure modest hardware and language support.** Qualify packaged apps
   on named 2 GB/4 GB configurations, integrated graphics, local fonts,
   multiple writing systems, screen readers, and an unavailable network.

Advanced morphing, batch generation, recording, and expanded exports should
follow these gates. On-device AI/live transcription has memory, download, and
power costs; it is not automatically suitable for this mission. No competitor
feature authorizes network access in 900Slides: application code must remain
free of telemetry, analytics, and remote calls.

## Maintaining the comparison

Refresh platform requirements and offline restrictions before a public release
or positioning change. Add benchmark claims only with a version, fixture,
hardware profile, method, and reproducible result. Keep visual similarity,
editability, package preservation, and successful reopening as separate
compatibility dimensions. Accessibility commitments need tested evidence before
becoming conformance claims.
