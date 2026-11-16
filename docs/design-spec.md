# Plume — visual design specification (macOS SwiftUI → HTML/CSS)

Extracted from the macOS Swift sources in `/Users/p.pocass/plume-app/Plume/Sources/Plume/`.
Nothing here is invented: every value carries a `File.swift:line` citation. Where a value
comes from a macOS system metric (a semantic colour, a text style, a material) the citation
points at the call site and the value is given as Apple documents it, followed by the hex used
by the web port.

Companion files: `docs/strings/en.json`, `docs/strings/fr.json` (see §6).

---

## 0. How to read this document

### 0.1 Citations and units

* `File.swift:line` = the exact line of the Swift source the value comes from.
* All geometry is in **points (pt)**, the unit SwiftUI and AppKit use. 1 pt = 1 CSS px at 100 %
  zoom; on a 2× Mac screen 1 pt = 2 device px.
* Colours are given as `#RRGGBB` (sRGB, as written in the Swift source) plus `rgba()` when the
  source uses an opacity multiplier.
* "Measured" means: sampled from the PNG captures in `docs/mac-captures/`. Those captures are
  **2× scale** (`1120×1144 px` = `560×572 pt` for the onboarding window). Measurements are
  rounded to the nearest 0.5 pt.

### 0.2 Capture caveat (important for colour work)

The PNG captures are stored in a wide-gamut (Display P3) profile. Neutral greys survive the
conversion exactly (`windowBackgroundColor` dark measures `#1E1E1E`, the notch card measures
`#0E0E0E`, both matching the source bit for bit), but **saturated colours drift**: the accent
`#4F46E5` measures `#3659DD` in `assistant-1-welcome-en-clair.png`, and the dark accent
`#8B93FF` measures `#7F9BF8`. **Use the hex from the source, never the sampled value**, for any
saturated colour. Greys sampled from captures are trustworthy and are marked "measured".

### 0.3 The app tint

The whole app is tinted once, at the root of every window:

```swift
.tint(.plumeTheme)            // MainWindow.swift:38
```

so every control that follows the accent colour (selected segment, focused ring, `.link`
buttons, prominent buttons, progress bars, the selected onboarding dot) uses `plumeTheme`.

**Observed exception** (from the captures): on macOS 26 a `.borderedProminent` button renders
as a *neutral* capsule even with the tint applied — light: fill `#EBEBEB`, label `#242424`;
dark: fill `#303030`, label `#DFDFDF`. Measured on the "Continue" button,
`assistant-1-welcome-en-clair.png` (81.5 × 28 pt capsule, radius 14) and
`assistant-1-welcome-en-sombre.png`. A web port that wants to match the *screenshots* should use
the neutral capsule; one that wants to match the *source* uses `#4F46E5` / `#8B93FF` with white
text. Both variants are listed in §1.2.6.

### 0.4 Text styles: macOS point sizes

SwiftUI text styles map to fixed macOS sizes (Apple HIG "Typography", macOS column). The ones
this app uses:

| SwiftUI | macOS size | Weight | Line height (default) |
|---|---|---|---|
| `.largeTitle` | 26 pt | regular | 32 pt |
| `.title` | 22 pt | regular | 26 pt |
| `.title2` | 17 pt | regular | 22 pt |
| `.title3` | 15 pt | regular | 20 pt |
| `.headline` | 13 pt | **semibold** | 16 pt |
| `.body` | 13 pt | regular | 16 pt |
| `.callout` | 12 pt | regular | 15 pt |
| `.subheadline` | 11 pt | regular | 14 pt |
| `.footnote` | 10 pt | regular | 13 pt |
| `.caption` | 10 pt | regular | 13 pt |
| `.caption2` | 10 pt | regular | 13 pt |

System font family on macOS is **SF Pro Text** below 20 pt and **SF Pro Display** at 20 pt and
above; for CSS use `-apple-system, "SF Pro Text", "Helvetica Neue", sans-serif`.
`.rounded` maps to **SF Pro Rounded** (`ui-rounded` / `font-family: ui-rounded`), `.monospaced`
to **SF Mono** (`ui-monospace`).

---

## 1. Global design system

### 1.1 Windows

#### 1.1.1 Main window

```
NSWindow(contentRect: 0,0,860,580)                       MainWindow.swift:42
styleMask: [.titled, .closable, .miniaturizable,
            .resizable, .fullSizeContentView]            MainWindow.swift:43
w.title = "Plume"                                        MainWindow.swift:46
w.toolbarStyle = .unified                                MainWindow.swift:47
w.minSize = 680 × 440                                    MainWindow.swift:48
w.setContentSize(860 × 580)                              MainWindow.swift:49
w.setFrameAutosaveName("PlumeMainWindow")                MainWindow.swift:52
if !w.setFrameUsingName("PlumeMainWindow") { w.center() } MainWindow.swift:53
controller.sceneBridgingOptions = [.toolbars, .title]    MainWindow.swift:41
```

* Default content size **860 × 580 pt**, minimum **680 × 440 pt**.
* Title bar is `.unified` (toolbar merged into the title bar), the content view extends under it
  (`.fullSizeContentView`), and the sidebar toggle is removed:
  `.toolbar(removing: .sidebarToggle)` (`MainWindow.swift:135`, macOS 15+).
* The window has a **real title** (`Plume`) and the navigation title follows the selected page:
  `.navigationTitle(navigation.selection.title)` (`MainWindow.swift:122`).
* Window background on the detail side: `Color(nsColor: .windowBackgroundColor)`
  (`MainWindow.swift:120`) — **measured `#FFFFFF` light / `#1E1E1E` dark**.
* The window is only created on demand and destroyed on close (activation policy flips to
  `.accessory`, `MainWindow.swift:57-66`).

#### 1.1.2 Sidebar column

```swift
NavigationSplitView {
    SidebarView()
        .navigationSplitViewColumnWidth(min: 190, ideal: 210, max: 280)   // MainWindow.swift:107
}
```

Sidebar width: **min 190 pt, ideal 210 pt, max 280 pt** (`MainWindow.swift:107`).

#### 1.1.3 Other windows

| Window | Size | Chrome | Source |
|---|---|---|---|
| Onboarding | 560 × 540 | `.titled, .closable, .fullSizeContentView`, transparent title bar, hidden title, movable by background | `Onboarding.swift:33-42` |
| What's New | 560 × 500 | same as onboarding | `WhatsNew.swift:90-96` |
| Vocabulary editor sheet | 440 × 440 | sheet | `VocabularyView.swift:190` |
| "Fix a Word" sheet | 420 wide, height intrinsic | sheet, padding 20 | `VocabularyView.swift:245-246` |
| Pill panel | card + 26 pt margins, see §3 | `.borderless, .nonactivatingPanel`, level `.statusBar` | `PillPanel.swift:26-62` |

All three "content" windows paint `Color(nsColor: .windowBackgroundColor)`
(`Onboarding.swift:106`, `WhatsNew.swift:159`).

### 1.2 Colours

#### 1.2.1 Brand accent — `Color.plumeTheme`

Dynamic colour, resolved per appearance (`Prefs.swift:410-419`):

| Appearance | Hex | rgba |
|---|---|---|
| Light | `#4F46E5` | `rgba(79, 70, 229, 1)` |
| Dark | `#8B93FF` | `rgba(139, 147, 255, 1)` |

The logo gradient the accent is derived from is `#7C8CFF → #4338CA` (`Prefs.swift:411-412`).
Used for: sidebar item icons (`MainWindow.swift:204`), the window tint (`MainWindow.swift:38`),
stat-tile icons and the "History" chevron link (`HomeView.swift:45`, `HomeView.swift:130`),
`Tag` accents on history rows (`HistoryView.swift:132`, `HistoryView.swift:135`), the selected
1337 level ring (`LeetView.swift:81-82`), onboarding dots (`Onboarding.swift:141`,
`WhatsNew.swift:145`), the "Press a key…" recording state (`SettingsView.swift:301`,
`SettingsView.swift:306`, `SettingsView.swift:311`), the update-available line
(`SettingsView.swift:472`), and the "New Word" affordance (`VocabularyView.swift:19`).

#### 1.2.2 "Done" green — `Color.plumeDone`

`Color(red: 95/255, green: 210/255, blue: 154/255)` → **`#5FD29A`**, fully opaque
(`Prefs.swift:425`). Used for: the sidebar engine dot when ready (`MainWindow.swift:302`),
permission "ok" icons (`HomeView.swift:196`), the pill ✓ (`PillPanel.swift:392`), the pill
"Copied" label (`PillPanel.swift:396`), "In use" labels (`ModelsView.swift:109`,
`ModelsView.swift:155`), "Allowed" in onboarding (`Onboarding.swift:371`), "Saved" API-key state
(`SettingsView.swift:431`).

#### 1.2.3 Pill tints — `PillTint` (12 choices)

`Prefs.swift:71-86`. Listening defaults to `cyan`, processing to `amber` (`Prefs.swift:330-331`).

| Case | Hex | rgb |
|---|---|---|
| cyan (default listening) | `#33D6E0` | 51, 214, 224 |
| blue | `#4D9FFF` | 77, 159, 255 |
| lavender | `#B4A7FF` | 180, 167, 255 |
| violet (label "Purple") | `#9D6BFF` | 157, 107, 255 |
| pink | `#FF77B7` | 255, 119, 183 |
| coral | `#FF8A7A` | 255, 138, 122 |
| red | `#FF5A5F` | 255, 90, 95 |
| amber (default processing) | `#F5A623` | 245, 166, 35 |
| orange | `#FF8C42` | 255, 140, 66 |
| yellow | `#F7D04A` | 247, 208, 74 |
| mint | `#5FD29A` | 95, 210, 154 |
| green | `#8BD957` | 139, 217, 87 |

#### 1.2.4 Semantic colours (Apple-documented values)

`Color.secondary`, `.primary`, `.tertiary`, `.quaternary`, `.orange`, `.red`, `.clear` are used
throughout instead of literals. Apple's documented sRGB values for the *labelled* colours:

| SwiftUI | AppKit equivalent | Light | Dark |
|---|---|---|---|
| `.primary` | `labelColor` | `rgba(0,0,0,0.85)` | `rgba(255,255,255,0.85)` |
| `.secondary` | `secondaryLabelColor` | `rgba(60,60,67,0.60)` | `rgba(235,235,245,0.60)` |
| `.tertiary` | `tertiaryLabelColor` | `rgba(60,60,67,0.30)` | `rgba(235,235,245,0.30)` |
| `.quaternary` | `quaternaryLabelColor` | `rgba(60,60,67,0.18)` | `rgba(235,235,245,0.16)` |
| `.orange` | `systemOrangeColor` | `#FF9500` | `#FF9F0A` |
| `.red` | `systemRedColor` | `#FF3B30` | `#FF453A` |
| `windowBackgroundColor` | `windowBackgroundColor` | `#FFFFFF` (measured) | `#1E1E1E` (measured) |
| `separatorColor` (Divider) | `separatorColor` | `rgba(0,0,0,0.10)` | `rgba(255,255,255,0.10)` |

Where the source applies an extra `.opacity(x)` **on top of** a semantic colour, the effective
alpha is the product. The ones that occur in this app:

| Expression | Light effective | Dark effective | Source |
|---|---|---|---|
| `.secondary.opacity(0.22)` (sidebar selection) | `rgba(60,60,67,0.132)` | `rgba(235,235,245,0.132)` | `MainWindow.swift:233` |
| `.secondary.opacity(0.3)` (unselected dot) | `rgba(60,60,67,0.18)` | `rgba(235,235,245,0.18)` | `Onboarding.swift:141`, `WhatsNew.swift:145` |
| `.secondary.opacity(0.5)` (1337 unselected ring) | `rgba(60,60,67,0.30)` | `rgba(235,235,245,0.30)` | `LeetView.swift:81` |
| `.quaternary.opacity(0.5)` (cards) | `rgba(60,60,67,0.09)` | `rgba(235,235,245,0.08)` | `HomeView.swift:52`, `:83`, `:140` |
| `.primary.opacity(0.08)` (keycap fill) | `rgba(0,0,0,0.068)` | `rgba(255,255,255,0.068)` | `HomeView.swift:116` |
| `.primary.opacity(0.15)` (keycap border) | `rgba(0,0,0,0.1275)` | `rgba(255,255,255,0.1275)` | `HomeView.swift:117` |
| `.primary.opacity(0.07)` (shortcut chip fill) | `rgba(0,0,0,0.0595)` | `rgba(255,255,255,0.0595)` | `Onboarding.swift:255` |
| `.primary.opacity(0.12)` (shortcut chip border) | `rgba(0,0,0,0.102)` | `rgba(255,255,255,0.102)` | `Onboarding.swift:256` |
| `.primary.opacity(0.04)` (text areas fill) | `rgba(0,0,0,0.034)` | `rgba(255,255,255,0.034)` | `LeetView.swift:135`, `:173` |
| `.primary.opacity(0.10)` (text area border) | `rgba(0,0,0,0.085)` | `rgba(255,255,255,0.085)` | `LeetView.swift:139` |
| `.primary.opacity(0.15)` (tint swatch ring) | see table above | — | `SettingsView.swift:231` |
| `.primary.opacity(0.75)` (selected swatch ring) | `rgba(0,0,0,0.6375)` | `rgba(255,255,255,0.6375)` | `SettingsView.swift:233` |
| `Color.orange.opacity(0.08)` (permissions card) | `#FF9500` @ 8 % | `#FF9F0A` @ 8 % | `HomeView.swift:170` |
| `Color.orange.opacity(0.25)` (permissions card border) | `#FF9500` @ 25 % | `#FF9F0A` @ 25 % | `HomeView.swift:171` |

#### 1.2.5 Literal colours

| Use | Value | Source |
|---|---|---|
| Onboarding / What's New icon tile gradient | `#7C8CFF` → `#4338CA`, top-leading → bottom-trailing | `Onboarding.swift:352-354` |
| Icon tile shadow | `#4338CA` @ 30 %, radius 10, y 4 | `Onboarding.swift:355` |
| Notch card fill (the card behind the notch shape) | `Color(white: 0.055)` = `#0E0E0E` opaque | `PillPanel.swift:372` |
| Pill card, notch style | `#0E0E0E` @ 97 % + white stroke 13 % | `PillPanel.swift:504-505` |
| Pill card, bottom/top, light | `.ultraThinMaterial` + white @ 30 % + white stroke 55 % | `PillPanel.swift:514-516` |
| Pill card, bottom/top, dark | `.ultraThinMaterial` + black @ 34 % + white stroke 20 % | `PillPanel.swift:515-516` |
| Pill inner hairline (bottom/top) | black @ 10 % light / 28 % dark, 0.5 pt, inset 0.5 | `PillPanel.swift:517` |
| Pill shadow (bottom/top) | black @ 22 % (light) / 42 % (dark), radius 14, y 5 + black @ 10 %, radius 2, y 1 | `PillPanel.swift:518-519` |
| Pill shadow (notch) | black @ 50 %, radius 20, y 9 | `PillPanel.swift:506` |
| Settings pill preview card | black @ 88 % + white stroke 14 %, 0.75 pt | `SettingsView.swift:265-266` |
| Text selection / caret | system accent | — |

#### 1.2.6 Button fills (measured, macOS 26)

| Style | Light fill / label | Dark fill / label | Source |
|---|---|---|---|
| `.borderedProminent` + `.controlSize(.large)` | `#EBEBEB` / `#242424` | `#303030` / `#DFDFDF` | `Onboarding.swift:149-152`, measured |
| `.borderedProminent` (default size, Settings/Models) | same family, default height | same | `ModelsView.swift:112`, `SettingsView.swift:200` |
| `.bordered` (default macOS push button) | `#FFFFFF` with `rgba(0,0,0,0.10)` border / `#000000` | `#3A3A3C` / `#FFFFFF` | `HomeView.swift:203`, `SettingsView.swift:437` |
| `.plain` (text button) | no fill, label `.primary` or `.plumeTheme` | — | `MainWindow.swift:223`, `HistoryView.swift:108` |
| `.borderless` (icon button) | no fill, label `.secondary`/`.primary` | — | `HistoryView.swift:157`, `VocabularyView.swift:141` |
| `.link` | no fill, label = accent, underlined on hover only | — | `HistoryView.swift:109`, `HistoryView.swift:117` |
| Prominent button geometry | capsule, **height 28 pt** for `.large`, radius = height/2 | | measured (button 81.5 × 28 pt) |

### 1.3 Materials and their CSS approximation

The app uses four materials. None of them is a flat colour; a web port needs
`backdrop-filter`. The approximations below are the ones to use when a real `backdrop-filter` is
available; the flat fallback is given for browsers without it.

| SwiftUI material | Where | CSS (light) | CSS (dark) | Flat fallback light / dark |
|---|---|---|---|---|
| Sidebar vibrancy (`.listStyle(.sidebar)` + `.scrollContentBackground(.hidden)`) | sidebar column, `MainWindow.swift:171-172` | `backdrop-filter: blur(30px) saturate(180%); background: rgba(246,246,246,0.72)` | `background: rgba(40,40,42,0.72)` | `#F2F2F2` / `#2A2A2C` |
| `.regularMaterial` | sidebar footer on macOS < 26, `MainWindow.swift:251` | `blur(20px) saturate(180%); rgba(255,255,255,0.72)` | `rgba(30,30,30,0.72)` | `#F7F7F7` / `#242424` |
| `.ultraThinMaterial` | pill card, `PillPanel.swift:514` | `blur(12px) saturate(180%); rgba(255,255,255,0.55)` | `rgba(40,40,40,0.45)` | `#EFEFEF` (measured over white) / `#3A3A3A` |
| `safeAreaBar` + `.scrollEdgeEffectStyle(.hard)` | sidebar footer on macOS 26, `MainWindow.swift:247-248` | hard scroll-edge: a solid band matching the sidebar tint, 1 px top hairline | same | — |

Measured composited values from the captures (useful sanity checks): the light pill card over
white renders `#EFEFEF` at its centre; the notch card renders exactly `#0E0E0E`.

### 1.4 Typography

Every `.font(...)` in the UI, with its resolved size/weight:

| Text | Font | Size / weight / design | Source |
|---|---|---|---|
| App name "Plume" (Home header) | `.system(size: 26, weight: .bold, design: .rounded)` | 26 pt bold rounded | `HomeView.swift:66` |
| Home subtitle | default `.body` + `.secondary` | 13 pt regular | `HomeView.swift:67-68` |
| Section titles on Home | `.headline` | 13 pt semibold | `HomeView.swift:34`, `HomeView.swift:75` |
| Stat value | `.system(size: 24, weight: .semibold, design: .rounded).monospacedDigit()` | 24 pt semibold rounded, tabular digits | `HomeView.swift:132` |
| Stat label | `.callout` | 12 pt regular | `HomeView.swift:135` |
| Gesture row text | default `.body` | 13 pt regular | `HomeView.swift:100` |
| Keycap | `.system(size: 11, weight: .semibold, design: .rounded)` | 11 pt semibold rounded | `HomeView.swift:113` |
| Permission detail | `.callout` | 12 pt regular | `HomeView.swift:199` |
| Permission row status icon | `.system(size: 18)` | 18 pt regular | `HomeView.swift:195` |
| Sidebar row label | `.body` (Label default) + `.semibold` when selected | 13 pt, regular/semibold | `MainWindow.swift:200` |
| Sidebar count badge | `.system(size: 11, weight: .medium).monospacedDigit()` | 11 pt medium, tabular | `MainWindow.swift:210` |
| Sidebar footer | `.system(size: 12)` | 12 pt regular | `MainWindow.swift:272` |
| History meta line | `.caption` | 10 pt regular | `HistoryView.swift:144` |
| History tag | `.system(size: 10, weight: .semibold)` | 10 pt semibold | `HistoryView.swift:202` |
| History "long text" link | `.link` + `.callout` | 12 pt, accent colour | `HistoryView.swift:109-110`, `:117-118` |
| Vocabulary word | `.body.weight(.semibold)` | 13 pt semibold | `VocabularyView.swift:37` |
| Vocabulary variants | `.callout` | 12 pt regular | `VocabularyView.swift:40` |
| Vocabulary row chevron | `.system(size: 11, weight: .semibold)` | 11 pt semibold | `VocabularyView.swift:47` |
| "New Word" affordance | `.body.weight(.medium)` | 13 pt medium | `VocabularyView.swift:18` |
| Vocabulary sheet word field | `.title3.weight(.semibold)` | 15 pt semibold | `VocabularyView.swift:121` |
| "Fix a Word" title | `.title3.weight(.semibold)` | 15 pt semibold | `VocabularyView.swift:219` |
| Sheet footnotes | `.caption` | 10 pt regular | `VocabularyView.swift:164`, `:231` |
| 1337 level name | default `.body` + `.medium` | 13 pt medium | `LeetView.swift:84` |
| 1337 level sample | `.system(.callout, design: .monospaced)` | 12 pt monospaced | `LeetView.swift:88` |
| "Result" caption | `.caption.weight(.semibold)` | 10 pt semibold | `LeetView.swift:145` |
| 1337 output | `.system(.body, design: .monospaced)` | 13 pt monospaced | `LeetView.swift:166` |
| Text area (AppKit) | `.monospacedSystemFont(ofSize: NSFont.systemFontSize, weight: .regular)` | 13 pt monospaced | `LeetView.swift:205` |
| Model name | `.body` + `.semibold` | 13 pt semibold | `ModelsView.swift:61` |
| Model summary | `.callout` | 12 pt regular | `ModelsView.swift:65` |
| Model facts / error | `.caption` | 10 pt regular | `ModelsView.swift:67`, `:69` |
| Model progress % | `.caption.monospacedDigit()`, width 34, trailing | 10 pt tabular | `ModelsView.swift:100` |
| Unsupported note | `.callout` | 12 pt regular | `ModelsView.swift:106` |
| Shortcut chip (Settings) | `.system(.body, design: .rounded).weight(.semibold)` | 13 pt semibold rounded | `SettingsView.swift:305` |
| Settings caption/footer hints | `.caption` | 10 pt regular | `SettingsView.swift:79`, `:336`, `:338` |
| Onboarding title | `.system(size: 26, weight: .bold)` | 26 pt bold | `Onboarding.swift:325` |
| Onboarding subtitle | `.body` | 13 pt regular | `Onboarding.swift:328` |
| Onboarding icon glyph | `.system(size: 34, weight: .medium)` | 34 pt medium, white | `Onboarding.swift:348` |
| Onboarding shortcut chip | `.system(size: 22, weight: .semibold, design: .rounded)` | 22 pt semibold rounded | `Onboarding.swift:253` |
| "Allowed" label | `.headline` | 13 pt semibold | `Onboarding.swift:370` |
| What's New eyebrow | `.callout.weight(.semibold)` | 12 pt semibold, secondary | `WhatsNew.swift:162` |
| Pill ✓ glyph | `.system(size: 13, weight: .bold)` | 13 pt bold | `PillPanel.swift:391` |
| Pill popup header icon | `.system(size: 11, weight: .semibold)` | 11 pt semibold | `PillPanel.swift:426` |
| Pill popup header text | `.system(size: 11, weight: .medium)` | 11 pt medium | `PillPanel.swift:429` |
| Pill popup body | `.system(size: 14)` | 14 pt regular | `PillPanel.swift:435` |
| Pill close ✕ (popup) | `.system(size: 7, weight: .heavy)` | 7 pt heavy | `PillPanel.swift:464` |
| Pill close ✕ (pill) | `.system(size: 7, weight: .bold)` | 7 pt bold | `PillPanel.swift:555` |
| Pill label (Copied / message) | `.system(size: 12, weight: .medium)` | 12 pt medium | `PillPanel.swift:614` |

### 1.5 Corner radii

| Radius | Use | Source |
|---|---|---|
| **5 pt** continuous | keycaps | `HomeView.swift:116-117` |
| **6 pt** continuous | shortcut chip (Settings) | `SettingsView.swift:308`, `:310` |
| **8 pt** continuous | sidebar selection row; 1337 text areas | `MainWindow.swift:232`, `LeetView.swift:134`, `:138`, `:172` |
| **10 pt** continuous | onboarding shortcut chip | `Onboarding.swift:255-256` |
| **12 pt** continuous | all Home cards (stat, gestures, last dictation, permissions) | `HomeView.swift:52`, `:83`, `:140`, `:170-171` |
| **14 pt** continuous | notch card bottom corners | `PillPanel.swift:628`, `:535-536` |
| **16 pt** continuous | pill popup rectangle | `PillPanel.swift:447`, `:540` |
| **20 pt** continuous | onboarding icon tile | `Onboarding.swift:351` |
| **capsule** | pill (bottom/top), tags, buttons, TintRow selection ring | `PillPanel.swift:539`, `HistoryView.swift:206` |

### 1.6 Paddings and spacings (the recurring numbers)

* Page content: **28 pt** on all sides, max content width **720 pt**, left-aligned
  (`HomeView.swift:56-58`).
* Card inner padding: **14 pt** (stat tiles, last dictation), **16 pt** (permissions card)
  (`HomeView.swift:139`, `HomeView.swift:51`, `HomeView.swift:169`).
* Card inner spacing: **6 pt** (stat), **12 pt** (gestures heading to list), **8 pt** (last
  dictation heading to row), **12 pt** (permissions heading to rows)
  (`HomeView.swift:128`, `:74`, `:32`, `:163`).
* Vertical rhythm between Home blocks: **22 pt** (`HomeView.swift:12`), stat tiles **14 pt**
  apart (`HomeView.swift:21`).
* Sidebar rows: vertical padding **5 pt**, insets top 1 / leading 10 / bottom 1 / trailing 10
  (`MainWindow.swift:220`, `MainWindow.swift:224`); selection pill inset **8 pt** horizontal,
  **2 pt** vertical (`MainWindow.swift:234-235`).
* Sidebar footer: horizontal **16 pt**, vertical **11 pt**, icon-to-text **7 pt**
  (`MainWindow.swift:264`, `:274-275`).
* Onboarding step page: horizontal **40 pt**, top spacer ≥30, icon → title **22 pt**, title →
  subtitle **8 pt**, subtitle → content **28 pt**, bottom spacer ≥10 (`Onboarding.swift:319-339`).
* Onboarding footer: horizontal **28 pt**, vertical **22 pt**, side buttons **110 pt** wide
  (`Onboarding.swift:154-155`, `:128`, `:135`, `:152`).
* Form sections: the app only supplies `Section { … } header: { Text(…) } footer: { Text(…) }`
  (`SettingsView.swift:30-38` is the canonical shape) and never overrides any metric — row
  height, header/footer type and insets belong to the macOS `.grouped` form. **Platform metric,
  not in the source**: rows ≈ 24 pt, section headers 11 pt semibold secondary, footers 10 pt
  secondary, ≈ 20 pt horizontal insets. Same for `.listStyle(.inset)` lists
  (`SettingsView.swift:150`, `ModelsView.swift:42`, `LeetView.swift:54`,
  `VocabularyView.swift:61-62`).

### 1.7 Shadows

| Element | Shadow | Source |
|---|---|---|
| Onboarding icon tile | `#4338CA` @ 30 %, radius 10, x 0, y 4 | `Onboarding.swift:355` |
| Pill, bottom/top, light | black @ 22 %, r 14, y 5 **and** black @ 10 %, r 2, y 1 | `PillPanel.swift:518-519` |
| Pill, bottom/top, dark | black @ 42 %, r 14, y 5 + black @ 10 %, r 2, y 1 | `PillPanel.swift:518-519` |
| Pill, notch | black @ 50 %, r 20, y 9 (defined in `pillBackground`, **not drawn** in the shipping notch card, cf. §3.9) | `PillPanel.swift:506` |
| Pill bars | own colour @ 50 %, r 4 | `PillPanel.swift:585` |
| Settings pill preview | colour @ 50 %, r 4 | `SettingsView.swift:262` |

No other element in the app casts a shadow.

### 1.8 Motion

| Animation | Curve | Duration / parameters | Source |
|---|---|---|---|
| Pill content (phase change) | `.spring(response: 0.32, dampingFraction: 0.78)` | — | `PillPanel.swift:358` |
| Pill bottom/top appearance | `.scale(scale: 0.6, anchor: .bottom)` combined with `.opacity` | spring above | `PillPanel.swift:345-347` |
| Pill slide in (top/notch) | ease-out cubic `1-(1-t)³`, window moved by a 60 Hz timer, start 4 pt above the top edge | **0.34 s** | `PillPanel.swift:184`, `:203`, `:219` |
| Pill slide out (top/notch) | ease-in quadratic `t²`, to `screen.maxY + 10` | **0.22 s** | `PillPanel.swift:243`, `:255` |
| Pill panel resize (same style) | `easeInEaseOut` | 0.22 s | `PillPanel.swift:166-167` |
| Pill panel re-place after a style change | `easeInEaseOut` | 0.28 s | `PillPanel.swift:139-140` |
| Onboarding page change | `.spring(response: 0.42, dampingFraction: 0.88)` + asymmetric move/opacity | — | `Onboarding.swift:201`, `:96-98` |
| What's New page change | same spring | — | `WhatsNew.swift:170` |
| Onboarding permission "Allowed" swap | `.easeOut(duration: 0.2)` | 0.2 s | `Onboarding.swift:377` |
| Stat value change | `.contentTransition(.numericText())` | system default | `HomeView.swift:133` |
| Pill preview bars (Settings) | 30 fps timeline | — | `SettingsView.swift:251` |
| Pill bars (real) | 30 fps timeline | — | `PillPanel.swift:573` |

### 1.9 SF Symbols, with the closest Lucide icon

| Symbol | Where | Size / weight | Colour | Lucide |
|---|---|---|---|---|
| `waveform` | sidebar Home (`MainWindow.swift:84`), Home "dictations" tile (`HomeView.swift:28`), History empty state (`HistoryView.swift:14`) | inherits (13 pt) | `plumeTheme` in sidebar, `.primary`/`.secondary` elsewhere | `audio-waveform` |
| `clock.arrow.circlepath` | sidebar History (`MainWindow.swift:85`), tray "History…" (`StatusItem.swift:59`) | inherits | `plumeTheme` | `history` |
| `character.book.closed` | sidebar Vocabulary (`MainWindow.swift:86`) | inherits | `plumeTheme` | `book-a` |
| `textformat.alt` | sidebar 1337 (`MainWindow.swift:87`), What's New 1.5 (`WhatsNew.swift:18`) | inherits | `plumeTheme` | `case-sensitive` |
| `cpu` | sidebar Models (`MainWindow.swift:88`), onboarding Model (`Onboarding.swift:284`), What's New 1.3 (`WhatsNew.swift:37`) | inherits / 34 pt in step icon | `plumeTheme` / white | `cpu` |
| `gearshape` | sidebar Settings (`MainWindow.swift:89`), tray "Settings…" (`StatusItem.swift:60`) | inherits | `plumeTheme` | `settings` |
| `exclamationmark.circle.fill` | sidebar Settings badge when a permission is missing (`MainWindow.swift:213`) | inherits | `.orange` | `circle-alert` |
| `chevron.right` | Home "History" link (`HomeView.swift:43`), vocabulary rows (`VocabularyView.swift:46`) | `.caption.weight(.semibold)` / 11 pt semibold | `plumeTheme` / `.tertiary` | `chevron-right` |
| `checkmark.circle.fill` | permission ok (`HomeView.swift:194`), "In use" (`ModelsView.swift:108`, `:155`), "Allowed" (`Onboarding.swift:369`), "Saved" (`SettingsView.swift:430`) | 18 pt in permission rows, else inherits | `plumeDone` | `circle-check` |
| `circle.dashed` | permission missing (`HomeView.swift:194`) | 18 pt | `.secondary` | `circle-dashed` |
| `exclamationmark.triangle.fill` | permissions card title (`HomeView.swift:164`), pill error message (`PillPanel.swift:398`) | inherits / 12 pt in pill | `.orange` | `triangle-alert` |
| `text.word.spacing` | Home "words today" tile (`HomeView.swift:23`) | inherits | `plumeTheme` | `case-upper` |
| `calendar` | Home "words in 7 days" tile (`HomeView.swift:25`) | inherits | `plumeTheme` | `calendar` |
| `trash` | History "Clear All" toolbar (`HistoryView.swift:41`), Models delete (`ModelsView.swift:114`) | toolbar default | `.primary` / `.secondary` | `trash-2` |
| `doc.on.doc` | History copy (`HistoryView.swift:154`), tray copy (`StatusItem.swift:53`) | 18×18 frame | `.secondary` | `copy` |
| `checkmark` | History copied state (`HistoryView.swift:154`), 1337 copied (`LeetView.swift:159`), pill pasted (`PillPanel.swift:390`) | 13 pt bold in pill | `plumeDone` | `check` |
| `doc.on.clipboard` / `doc.on.clipboard.fill` | 1337 copy button (`LeetView.swift:159`), pill copied label (`PillPanel.swift:395`) | 12 pt medium in pill | `plumeDone` | `clipboard` / `clipboard-check` |
| `plus` | Vocabulary toolbar (`VocabularyView.swift:66`), add form (`VocabularyView.swift:149`) | toolbar default | `.primary` | `plus` |
| `plus.circle.fill` | Vocabulary "New Word" row (`VocabularyView.swift:17`) | 13 pt medium | `plumeTheme` | `circle-plus` |
| `minus.circle.fill` | Vocabulary remove form (`VocabularyView.swift:139`) | inherits | `.secondary` | `circle-minus` |
| `mic.fill` | onboarding Microphone (`Onboarding.swift:234`) | 34 pt medium | white on tile | `mic` |
| `mic` | tray when idle (`StatusItem.swift:47`), What's New 1.4 (`WhatsNew.swift:32`) | menu icon | template | `mic` |
| `accessibility` | onboarding Accessibility (`Onboarding.swift:241`) | 34 pt | white | `accessibility` |
| `keyboard` | onboarding Shortcut (`Onboarding.swift:249`) | 34 pt | white | `keyboard` |
| `globe` | onboarding Spoken language (`Onboarding.swift:274`) | 34 pt | white | `globe` |
| `sparkles` | onboarding Cleanup (`Onboarding.swift:291`), What's New 1.3 (`WhatsNew.swift:43`) | 34 pt | white | `sparkles` |
| `tray.and.arrow.down.fill` | pill popup header (`PillPanel.swift:425`) | 11 pt semibold | `plumeAccent` | `download` |
| `waveform.slash` | pill message, non-error (`PillPanel.swift:398`) | 12 pt | `.secondary` | `audio-waveform` (slash variant: `audio-lines` + slash) |
| `xmark` | pill cancel (`PillPanel.swift:554`), popup close (`PillPanel.swift:463`) | 7 pt bold / heavy | `.primary` 62 % / 55 % | `x` |
| `stop.circle` | tray while recording (`StatusItem.swift:47`) | menu icon | template | `circle-stop` |
| `xmark.rectangle` | tray "Quit Plume" (`StatusItem.swift:69`) | menu icon | template | `rectangle-x` |
| `cup.and.saucer` | tray "Support Plume…" (`StatusItem.swift:61`) | menu icon | template | `coffee` |
| `circle.lefthalf.filled` | appearance "System" (`L10n.swift:54`) — symbol declared, not drawn in any view | 13 pt | — | `circle-half` |
| `sun.max` | appearance "Light" (`L10n.swift:56`) — declared only | — | — | `sun` |
| `moon` | appearance "Dark" (`L10n.swift:57`) — declared only | — | — | `moon` |
| `rectangle.topthird.inset.filled` | What's New 1.5 page 2 (`WhatsNew.swift:21`) | 34 pt | white | `panel-top` |
| `forward.end` | What's New 1.5 page 3 (`WhatsNew.swift:24`) | 34 pt | white | `skip-forward` |
| `text.append` | What's New 1.4 (`WhatsNew.swift:29`) | 34 pt | white | `text-cursor-input` |
| `arrow.down.circle` | What's New 1.3 (`WhatsNew.swift:40`) | 34 pt | white | `circle-arrow-down` |
| `menubar.rectangle` | What's New 1.3 (`WhatsNew.swift:43`) | 34 pt | white | `panel-top` (no closer match) |

The tray bar icon is **not** an SF Symbol: it is a hand-drawn 18 × 18 pt template image (a
feather, two stroked paths, 1.3 pt lines, round caps/joins) — `StatusItem.swift:88-110`, drawn
from `Tools/icone-barre-menus/B.svg`. For the web port use a 18 × 18 SVG with
`stroke-width: 1.3`, `stroke-linecap: round`, `stroke-linejoin: round`, currentColor.

---

## 2. Screens

Each section gives: the container, the layout tree (top to bottom), every control with its label
key (the key in `docs/strings/en.json`), its style and its geometry, then the conditional states.
"Key" values are quoted as `key` without quotes to keep the tables readable; the JSON holds the
localised text.

### 2.1 Sidebar (always visible)

Container: `NavigationSplitView` sidebar column, 190 / 210 / 280 pt wide
(`MainWindow.swift:105-107`). Content is a `List` with **no** `List(selection:)` — the row is
drawn by hand, because the native selection changes appearance when the window loses focus
(`MainWindow.swift:150-153`).

```
List  (.listStyle(.sidebar), .scrollContentBackground(.hidden))          MainWindow.swift:160-172
├── Section
│   └── ForEach(SidebarItem.allCases)         6 rows, in this order:
│       Home "waveform" · History "clock.arrow.circlepath" · Vocabulary "character.book.closed"
│       1337 "textformat.alt" (hard-coded title, MainWindow.swift:77) · Models "cpu"
│       Settings "gearshape"                   (MainWindow.swift:74-90)
│       └── SidebarRow                                  MainWindow.swift:185-238
│           HStack(spacing: 0)
│           ├── Label
│           │   ├── Text(title)  .lineLimit(1)
│           │   │     fontWeight: .semibold if selected else .regular   :200
│           │   │     .foregroundStyle(.primary)                        :201
│           │   └── Image(systemName:) .foregroundStyle(.plumeTheme)    :203-204
│           ├── Spacer(minLength: 6)                                    :206
│           └── badge: Text(n) 11 pt medium monospacedDigit .secondary   :208-211
│                   or Image("exclamationmark.circle.fill") .orange      :212-214
│           .frame(maxWidth: .infinity, alignment: .leading)             :219
│           .padding(.vertical, 5)                                       :220
│           .contentShape(Rectangle())
│           .buttonStyle(.plain)                                         :223
│           .listRowInsets(1, 10, 1, 10)                                 :224
│           .listRowBackground(selection pill when selected)             :225
└── footer: safe-area bar, always visible                                MainWindow.swift:242-306
```

Selection pill (only when the row is selected, `MainWindow.swift:229-237`):

```
RoundedRectangle(cornerRadius: 8, style: .continuous)
    .fill(Color.secondary.opacity(0.22))      // rgba(60,60,67,.132) light / rgba(235,235,245,.132) dark
    .padding(.horizontal, 8)
    .padding(.vertical, 2)
```

Badges (`MainWindow.swift:176-182`):
* Settings gets the **warning** badge when `!permissions.allGood`.
* History gets the **count** badge with the number of stored dictations (0 → no badge).
* All other rows: no badge.

Sidebar footer (`MainWindow.swift:259-306`) — the engine state line:

```
HStack(spacing: 7)
├── Circle().fill(dotColor).frame(width: 7, height: 7)
├── Text(shortLabel).lineLimit(1)
└── Spacer(minLength: 0)
.font(.system(size: 12)) .foregroundStyle(.secondary)
.padding(.horizontal, 16).padding(.vertical, 11)
.overlay(alignment: .top) { Divider().padding(.horizontal, 12) }
```

`dotColor` (`MainWindow.swift:297-305`):

| State | Dot | Label key |
|---|---|---|
| model downloading | `plumeThinking` (amber by default) | `downloading_percent` |
| engine sleeping | `secondary` @ 50 % | `name_idle` |
| loading / not installed | `plumeThinking` | `name_loading` / `model_not_downloaded` |
| ready (local) | `plumeDone` | `name_ready` |
| ready (shared engine) | `plumeDone` | `name_ready_shared` |
| online transcription | `plumeDone` | `online_transcription` |
| failed | `.red` | `name_error` |

The footer also carries `.help(engine.state.label)` (`MainWindow.swift:277`), whose strings are
`idle`, `loading`, `model_not_downloaded`, `ready`, `ready_shared`, `online`, `error`.

Background: the native sidebar vibrancy shows through because the list background is hidden
(`MainWindow.swift:172`); on macOS 26 the footer is a native `safeAreaBar` with
`.scrollEdgeEffectStyle(.hard, for: .bottom)` (`MainWindow.swift:247-248`); before macOS 26 it is
a `safeAreaInset` filled with `.regularMaterial` (`MainWindow.swift:250-252`).

### 2.2 Home (`HomeView.swift`)

Container: `ScrollView` → `VStack(alignment: .leading, spacing: 22)`, padding **28 pt**,
`.frame(maxWidth: 720, alignment: .leading)` then `.frame(maxWidth: .infinity)`
(`HomeView.swift:11-58`). So: content column 720 pt wide, centred in the detail pane, left-aligned
inside itself.

```
ScrollView
└── VStack(leading, 22)                                      :12
    ├── header                                              :62-71
    │   HStack(spacing: 16)
    │   ├── AppGlyph(size: 58)  → app icon laid out at 58 × 1.24 = 71.9 pt inside a 58 pt
    │   │                          frame (the icon file has its own shadow margin; not clipped)  :64,147-156
    │   └── VStack(leading, 4)
    │       ├── Text("Plume") 26 pt bold rounded            :66
    │       └── Text(voice_dictation_in_every_app) .secondary :67-68
    ├── PermissionsCard            (only if !permissions.allGood)   :15-17
    ├── gestures                                            :73-85
    ├── HStack(spacing: 14) — 3 × StatTile                   :21-29
    └── "Latest" block             (only if a non-failed dictation exists)  :31-54
```

**Permissions card** (`HomeView.swift:159-173`), conditional on `!permissions.allGood`:

```
VStack(leading, 12)
├── Label(permissions_required, "exclamationmark.triangle.fill") .headline .orange   :164-166
└── PermissionRows
    ├── PermissionRow(microphone, to_hear_your_voice, ok: mic == .authorized)          :179-180
    └── PermissionRow(accessibility, for_the_shortcut_and_text_insertion, ok:)        :181-182
.padding(16)
.background(Color.orange.opacity(0.08), RoundedRectangle 12 continuous)               :169-170
.overlay(RoundedRectangle 12 continuous .strokeBorder(Color.orange.opacity(0.25)))    :171
```

`PermissionRow` (`HomeView.swift:186-207`): `HStack(spacing: 12)` → status icon 18 pt
(`checkmark.circle.fill` in `plumeDone` when ok, `circle.dashed` in `.secondary` otherwise),
`VStack(leading, 2)` with title `.medium` and detail `.callout`/`.secondary`, `Spacer()`, and when
not ok a default-style `Button(allow)`.

**How to use** (`HomeView.swift:73-85`):

```
VStack(leading, 12)
├── Text(how_to_use) .headline                                                    :75
└── VStack(spacing: 0)                                                            :76
    ├── GestureRow(keys: [shortcut.label], start_dictation)                        :77
    ├── Divider().padding(.leading, 138)                                           :78
    ├── GestureRow(keys: [shortcut.label], stop_and_insert_text)                    :79
    ├── Divider().padding(.leading, 138)                                           :80
    ├── GestureRow(keys: [shortcut.label, "×2"], cancel)                            :81
    .background(.quaternary.opacity(0.5), RoundedRectangle 12 continuous)          :83
```

`GestureRow` (`HomeView.swift:88-106`): `HStack(spacing: 14)` → key stack (`HStack(spacing: 4)`
of `KeyCap`s) in a **110 pt wide leading-aligned frame**, then the text, then `Spacer()`; padding
horizontal 14, vertical 11. `KeyCap` (`HomeView.swift:108-120`): 11 pt semibold rounded, padding
6 / 3, radius **5**, fill `primary @ 8 %`, 1 pt border `primary @ 15 %`, `.fixedSize()`.

**Stat tiles** (`HomeView.swift:21-29`, `122-142`): three equal tiles, `HStack(spacing: 14)`, each
`VStack(leading, 6)` = symbol (`plumeTheme`) / value / label, `.frame(maxWidth: .infinity,
alignment: .leading)`, padding 14, radius 12, `quaternary @ 50 %`.

| Tile | Value | Label key | Symbol |
|---|---|---|---|
| 1 | words dictated since local midnight | `words_today` | `text.word.spacing` |
| 2 | words in the last 7 days (−7 × 86 400 s) | `words_in_7_days` | `calendar` |
| 3 | total dictations ever | `dictations` | `waveform` |

Values are formatted with the app language, not the system locale (`L10n.swift:21-23`), and
animate with `.contentTransition(.numericText())` (`HomeView.swift:133`).

**Latest dictation** (`HomeView.swift:31-54`), only when a dictation without `failed` exists —
uses the first matching item of the list (newest first):

```
VStack(leading, 8)
├── HStack
│   ├── Text(latest) .headline                                             :34
│   ├── Spacer()
│   └── Button → navigation.selection = .history
│       HStack(spacing: 3){ Text(history); Image("chevron.right").caption.semibold }
│       .foregroundStyle(plumeTheme)  .buttonStyle(.plain)  .focusable(false)  :38-48
└── HistoryRow(item:)                                                      :50
    .padding(14)
    .background(.quaternary.opacity(0.5), RoundedRectangle 12 continuous)   :51-52
```

### 2.3 History (`HistoryView.swift`)

Container: `Group` switching between three exclusive states (`HistoryView.swift:11-34`), plus a
toolbar search field and a toolbar "clear" button.

```
Group
├── if history.items.isEmpty       → ContentUnavailableView                          :12-17
│       Label(no_dictations, "waveform") + description Text(press_shortcut_to_start)
├── else if filtered.isEmpty       → ContentUnavailableView.search(text: search)      :18-19
└── else → List (.listStyle(.inset), .scrollContentBackground(.hidden))               :21-32
        └── ForEach(groups) Section(group.label)         one section per day, newest first
            └── ForEach(group.items) HistoryRow(item:)
                    .padding(.vertical, 6)                                            :26
.searchable(text:, placement: .toolbar, prompt: search)                               :35
.toolbar { ToolbarItem(.primaryAction) Button { } label: Label(clear_all, "trash") }   :36-45
```

* Day sections are labelled `today`, `yesterday`, or a localised date built with the template
  `EEEEdMMMM`, first letter upper-cased (`HistoryView.swift:72-81`).
* `clear_all` toolbar button is `.disabled(history.items.isEmpty)` and has `.help(clear_all)`
  (`HistoryView.swift:43-44`). It opens a confirmation dialog (`HistoryView.swift:47-52`):
  title `clear_all_history`, destructive button `clear_all`, cancel `cancel`, message
  `count_dictations_will_be_deleted`.

**History row** (`HistoryView.swift:84-195`):

```
HStack(alignment: .top, spacing: 12)                                    :96
├── VStack(leading, 6)                                                   :97
│   ├── text zone, one of:
│   │   ├── failed → Text(couldn_t_transcribe_seconds_s_of_audio_kept) .secondary   :98-100
│   │   ├── long and collapsed (text.count > 420)                        :93,101-110
│   │   │     Text(text).lineLimit(6).fixedSize(v:true).onTapGesture{expanded=true}
│   │   │     Button(show_all) .buttonStyle(.link) .font(.callout)
│   │   └── normal → Text(text).textSelection(.enabled).fixedSize(v:true)            :111-114
│   │         + if long: Button(show_less) .link .callout                            :115-119
│   └── meta HStack(spacing: 8) .font(.caption) .foregroundStyle(.secondary)         :121-145
│       ├── Text(date, style: .time)
│       ├── if appName: Text("·") Text(appName)
│       ├── if !failed: Text("·") Text(count_words)
│       ├── if !pasted && !failed: Tag(not_inserted, plumeTheme)
│       ├── if leet: Tag("1337", plumeTheme)
│       ├── if polished: Tag(cleaned, .secondary)
│       └── if failed: Tag(failed, .orange)
├── Spacer(minLength: 8)                                                 :147
└── trailing action                                                      :148-160
    ├── failed → Button(retry)                                           :149
    └── else → Button { copy } label: Image(copied ? "checkmark" : "doc.on.doc")
           .frame(18×18) .buttonStyle(.borderless) .help(copy)
           .opacity(hovering || copied ? 1 : 0)                           :159
```

`Tag` (`HistoryView.swift:197-207`): 10 pt semibold, padding horizontal 6 / vertical 2, colour,
`Capsule()` filled with the same colour at **14 %**.

Hover: `.onHover { hovering = $0 }` on the whole row (`HistoryView.swift:163`); the copy button
fades in (and stays for 1.2 s after a copy, `HistoryView.swift:189-193`).

Context menu (`HistoryView.swift:165-183`), in order: `copy`, `copy_raw_text` (only if the text
was cleaned/1337'd and differs from `raw`), separator, `fix_a_word`, `rewrite_as_1337` /
`back_to_normal_text`, separator, `delete` (destructive).

### 2.4 Vocabulary (`VocabularyView.swift`)

```
List (.listStyle(.inset), .scrollContentBackground(.hidden))                  :12,61-62
├── Section                                                                   :13
│   ├── Button → new entry        Label(new_word, "plus.circle.fill")          :14-22
│   │     .font(.body.weight(.medium)) .foregroundStyle(plumeTheme)
│   │     .buttonStyle(.plain) .padding(.vertical, 4)
│   └── Text(write_how_it_sounds_the_way_you_hear_it_plume_picks_up_every_variant)
│         .font(.callout) .secondary .fixedSize(v: true)                      :23-27
└── Section(header: count_words  e.g. "12 words")                             :30
    └── ForEach(filtered) Button → editing = entry                            :31-58
        HStack(alignment: .firstTextBaseline)                                 :35
        ├── VStack(leading, 3)
        │   ├── Text(word) .body.semibold                                     :37
        │   └── if variants: Text(heard + joined ", ") .callout .secondary .lineLimit(2)  :38-43
        ├── Spacer()
        └── Image("chevron.right") 11 pt semibold .tertiary                    :46-48
        .padding(.vertical, 5) .contentShape(Rectangle())                      :50-51
        .buttonStyle(.plain)                                                   :53
        .contextMenu { edit_2 ; delete(destructive) }                          :54-57
.searchable(placement: .toolbar, prompt: search)                               :63
.toolbar { ToolbarItem(.primaryAction) Label(new_word, "plus") .help(new_word) } :64-69
.sheet(item: $editing) { VocabEditorSheet }                                     :70-72
```

**Vocabulary editor sheet** (`VocabularyView.swift:84-207`), fixed **440 × 440**:

```
VStack(leading, 0)
├── Form (.formStyle(.grouped))
│   ├── Section(word)
│   │   └── TextField(word, prompt: correct_spelling) .title3.semibold  .focused   :119-124
│   ├── Section(how_it_sounds)
│   │   ├── ForEach(forms): HStack
│   │   │     TextField(form, prompt: e_g_dipsik).labelsHidden()
│   │   │     + Button(remove) Image("minus.circle.fill") .secondary .borderless   :127-145
│   │   ├── Button(add) Label(add, "plus") .borderless                              :146-151
│   │   └── footer Text(one_per_line_using_the_letters_you_actually_hear_plume_replaces_every_one_of_them_with_the_word_above)
│   │             .caption .secondary                                               :162-165
├── HStack (padding horizontal 20, bottom 16)                                        :170-188
│   ├── if !isNew: Button(delete_word, role: .destructive)                           :171-175
│   ├── Spacer()
│   ├── Button(cancel, role: .cancel) .keyboardShortcut(.cancelAction)                :178-179
│   └── Button(save) .keyboardShortcut(.defaultAction) .disabled(word empty)          :180-185
```

**"Fix a Word" sheet** (`VocabularyView.swift:211-247`), width **420**, padding **20**:

```
VStack(leading, 14)
├── Text(fix_a_word_2) .title3.semibold                                   :219
├── Text(item.text) .callout .secondary .lineLimit(4) .textSelection(.enabled)  :220-224
├── Form(.columns) { TextField(heard_2) ; TextField(correct) }            :225-229
├── Text(fixed_here_and_added_to_vocabulary) .caption .secondary           :230-232
└── HStack { Spacer(); Button(cancel, .cancelAction); Button(fix, .defaultAction) .disabled(either field empty) }  :233-243
```

### 2.5 1337 (`LeetView.swift`)

A native grouped `Form` (`.formStyle(.grouped)`, `.scrollContentBackground(.hidden)`,
`LeetView.swift:54-55`), four sections.

```
Form
├── Section(header: 1337_dictation, footer: dictate_as_usual_then_finish_...)          :21-33
│   ├── Toggle(enable_1337_dictation, isOn: $prefs.leetEnabled)                         :22
│   └── LabeledContent(shortcut) { ShortcutRecorder(allowsPairs: true) }                :23-25
├── Section(header: level, footer: links_email_addresses_are_never_changed_...)         :35-44
│   ├── LevelRow(.light,  sample, $prefs.leetLevel)      label = light_2                :36
│   ├── LevelRow(.medium, sample, $prefs.leetLevel)      label = medium                 :37
│   └── LevelRow(.full,   sample, $prefs.leetLevel)      label = full                   :38
└── Section(header: convert_some_text, footer: paste_some_text_and_it_comes_out_...)    :45-52
    └── LeetTransformer()                                                               :46
```

**LevelRow** (`LeetView.swift:67-100`) — a clickable row whose radio dot is drawn in the accent:

```
Button { selection = level } label:
  VStack(leading, 4)
  ├── HStack(6)
  │   ├── Circle().strokeBorder(isSelected ? plumeTheme : secondary.opacity(0.5), lineWidth: 2)
  │   │     .background(Circle().fill(isSelected ? plumeTheme : .clear))
  │   │     .frame(10 × 10)                                                  :80-83
  │   ├── Text(level.label).fontWeight(.medium)                              :84
  │   └── Spacer()
  └── Text(Leet.transform(sample, level:)) .system(.callout, .monospaced) .secondary
        .textSelection(.enabled).fixedSize(v: true)                          :87-91
  .padding(.vertical, 4) .contentShape(Rectangle())                          :93-94
.buttonStyle(.plain) .focusable(false)
.accessibilityAddTraits(isSelected ? .isSelected : [])                       :96-98
```

The sample sentence is `the_quick_brown_fox_jumps_over_the_lazy_dog`
(`LeetView.swift:14-17`), transformed live per level.

**LeetTransformer** (`LeetView.swift:112-179`) — the converter:

```
VStack(leading, 10)
├── PlaceholderTextView(text:, placeholder: paste_or_type_text_here)         :129-131
│     .frame(minHeight: 96)                                                  :132
│     .background(RoundedRectangle 8 continuous .fill(primary @ 4 %))         :133-136
│     .overlay(RoundedRectangle 8 continuous .strokeBorder(primary @ 10 %))   :137-140
├── if !input.isEmpty
│   ├── HStack { Text(result) .caption.semibold .secondary ; Spacer()
│   │            Button { copy } label: Label(copied ? copied : copy,
│   │                    systemImage: copied ? "checkmark" : "doc.on.clipboard")
│   │            .buttonStyle(.borderless) .disabled(output.isEmpty) }        :143-163
│   └── Text(output) .system(.body, .monospaced).textSelection(.enabled)
│         .fixedSize(v: true).frame(maxWidth: .infinity, leading)
│         .padding(10) .background(RoundedRectangle 8 .fill(primary @ 4 %))    :165-174
.padding(.vertical, 2)                                                        :177
```

The text area is a real `NSTextView` (`LeetView.swift:191-291`): monospaced system font at
`NSFont.systemFontSize` (13 pt) (`LeetView.swift:205`), `textContainerInset = (9, 10)`
(`LeetView.swift:196`), `lineFragmentPadding = 0` (`LeetView.swift:211`), transparent
background, vertical scroller with autohide. The placeholder is a separate `NSTextField` label
positioned at the text container origin, `.placeholderTextColor`, single line, truncating tail
(`LeetView.swift:221-283`).

### 2.6 Models (`ModelsView.swift`)

Grouped `Form` (`.formStyle(.grouped)`, `.scrollContentBackground(.hidden)`,
`ModelsView.swift:42-43`), four sections.

```
Form
├── Section(footer: everything_runs_on_your_mac_your_voice_never_leaves_it)          :10-14
│   └── LabeledContent(this_mac, value: "<chip> · <memory> GB")                     :11
├── Section(header: "Parakeet", footer: runs_on_the_neural_engine_very_fast_even_with_8_gb)  :16-22
│   └── ForEach(parakeet models) ModelRow(model:)                                    :17
├── Section(header: "Whisper", footer: runs_on_the_gpu_more_languages_and_it_uses_your_vocabulary)  :24-31
│   └── ForEach(whisper models) ModelRow(model:)                                     :25
└── Section(header: online, footer: with_your_own_api_key_your_voice_is_sent_to_the_chosen_service)  :33-40
    └── CloudModelSection()                                                          :34
```

**ModelRow** (`ModelsView.swift:48-127`):

```
HStack(alignment: .center, spacing: 12)                                              :58
├── VStack(leading, 4)                                                               :59
│   ├── HStack(6) { Text(displayName).semibold
│   │              if recommended: Tag(recommended_for_this_mac, plumeTheme) }        :60-63
│   ├── if !compact Text(summary) .callout .secondary                                 :64-66
│   ├── Text(facts) .caption .secondary                                               :67
│   │     facts = "<size> · <languages> · speed: <s> · accuracy: <a> · <mem> GB memory recommended"
│   └── if error Text(error) .caption .red                                            :68-70
├── Spacer(minLength: 8)
└── action                                                                            :95-122
.padding(.vertical, 3) .confirmationDialog(delete_model)                              :75-82
```

`action` states, in priority order (`ModelsView.swift:96-121`):

| Condition | What is drawn |
|---|---|
| download in progress | `ProgressView(value:)` 80 pt wide + `"NN %"` 10 pt tabular in a 34 pt trailing frame + `Button(cancel)` |
| unsupported | `Text(requires_macos_version)` or `Text(unavailable)`, `.callout` `.secondary` |
| installed **and** selected | `Label(in_use, "checkmark.circle.fill")` in `plumeDone` |
| installed, not selected | `Button(use)` + icon-only `Button(Image("trash"))` `.borderless` `.help(delete)` when deletable |
| not installed | `Button(download)` |

Confirmation dialog: title `delete_model` ("Delete <name>?"), destructive `delete`, cancel
`cancel`, message `size_will_be_freed_you_can_download_it_again`.

**CloudModelSection** (`ModelsView.swift:130-161`), also reused in onboarding:

```
Picker(service, $prefs.sttService)                                    :138-141
if service == .custom: TextField(url, prompt: "https://…/v1")          :142-144
TextField(model, prompt: service.defaultModel)                        :145
APIKeyRow(account:)                                                   :146
ConnectionTestRow { CloudTranscriber.test() }                         :147
if let page: Link(create_a_service_key, destination: page)            :148-150
if showUseButton: HStack { Spacer(); if selected: Label(in_use, "checkmark.circle.fill").plumeDone
                           else Button(use_online_transcription) }    :151-160
```

### 2.7 Settings (`SettingsView.swift`)

Grouped `Form`, `.formStyle(.grouped)`, `.scrollContentBackground(.hidden)`
(`SettingsView.swift:150-151`). Ten sections, in this order:

| # | Header | Rows | Source |
|---|---|---|---|
| 1 | `general` | `Picker(language)` · `Picker(appearance)` · `Toggle(open_at_login)` · `Toggle(show_in_menu_bar)` · `Toggle(sounds)` | `:14-28` |
| 2 | `shortcut` (footer = `prefs.shortcut.explanation`) | `LabeledContent(shortcut)` → `ShortcutRecorder` | `:30-38` |
| 3 | `dictation` | `Picker(spoken_language)` · `MicrophonePicker()` · `Toggle(voice_commands)`; footer = the voice-command list when enabled, plus `with_whisper_choosing_the_language_is_more_reliable` when the active model is Whisper and language is `.auto` | `:40-57` |
| 4 | `cleanup` (footer `fixes_punctuation_and_filler_words_only_text_is_sent_with_your_own_key`) | `Toggle(clean_up_text)` · `PolishServiceRows()` | `:59-67` |
| 5 | `indicator_style` (footer `the_indicator_takes_no_room_in_the_menu_bar_...`) | segmented `Picker` (labels hidden) over the three `PillStyle`s + `Text(style.detail)` `.caption` `.secondary` | `:69-86` |
| 6 | `colors` | `TintRow(listening)` · `TintRow(processing)` · `LabeledContent(preview)` → `HStack(10)` of two `PillPreview`s | `:88-97` |
| 7 | `engine` (footer conditional) | `LabeledContent(status, value: engine.state.label)` · `LabeledContent(model)` + `Button(change)` · `Picker(sleep_after)` | `:99-119` |
| 8 | `permissions` | `PermissionRows()` · if accessibility granted but hotkey inactive: `Text(activating_shortcut)` `.callout` `.secondary` | `:121-127` |
| 9 | `updates` (footer conditional) | `LabeledContent(version)` · if supported: `Toggle(check_automatically)` + `LabeledContent(update)` → `UpdateStatus` | `:129-141` |
| 10 | — | `Button(run_setup_again)` · `Button(report_a_problem)` | `:143-148` |

All pickers carry `.focusable(false)` so no grey focus ring is drawn
(`SettingsView.swift:18`, `:22`, `:44`, `:77`, `:112`, `:167`, `:181`, `:507`).

**Condensed pickers used in this screen**
* `language`: menu with `Français` / `English` (`L10n.swift:9`).
* `appearance`: menu with `system` / `light` / `dark` (`L10n.swift:44-50`).
* `spoken_language`: menu with `automatic_detection_recommended`, `french`, `english`, `spanish`,
  `german`, `italian`, `portuguese` (`Prefs.swift:9-18`).
* `sleep_after`: menu built from `Prefs.idleChoices = [15, 35, 60, 120, 300, 900, -1]`
  (`Prefs.swift:293`, `SettingsView.swift:107-111`); labels `"15 s"`, `"35 s"`, `"1 min"`,
  `"2 min"`, `"5 min"`, `"15 min"`, `never`.
* `service` (cleanup): `PolishService.available` labels (`SettingsView.swift:164-166`).
* `instructions`: `automatic` / `Français` / `English` (`Prefs.swift:148-152`).

**ShortcutRecorder** (`SettingsView.swift:281-342`) — the recorder used by Settings, 1337 and
onboarding:

```
VStack(alignment: compact ? .center : .trailing, spacing: 6)                 :298
├── HStack(spacing: 8)                                                       :299
│   ├── if compact && recording: Text(press_a_key) .foregroundStyle(plumeTheme)   :300-302
│   ├── if !compact: the current key as a chip:                               :303-312
│   │     Text(recording ? press_a_key : shortcut.label)
│   │       .system(.body, design: .rounded).weight(.semibold)
│   │       .foregroundStyle(recording ? plumeTheme : .primary)
│   │       .padding(horizontal 10, vertical 3)
│   │       .background(RoundedRectangle 6 .fill(primary @ 4 % (recording) / 8 %))
│   │       .overlay(RoundedRectangle 6 .stroke(recording ? plumeTheme : .clear, 1))
│   ├── Button(recording ? cancel : change_2)                                :313-315
│   └── Menu(presets) { preset.label… } .fixedSize()                          :316-329
└── if recording: Text(single_key_or_combo_... / single_key_combo_...) .caption .secondary  :331-336
    else if problem: Text(problem) .caption .orange                          :337-339
```

Recording ends on Esc (`SettingsView.swift:408`); a pair of modifiers registers at once when
`allowsPairs` (`SettingsView.swift:371-377`).

**TintRow** (`SettingsView.swift:217-243`):

```
LabeledContent(title) { HStack(spacing: 5) { ForEach(PillTint.allCases) Button {
    Circle().fill(tint.color).frame(16 × 16)
        .overlay(Circle().stroke(primary @ 15 %, 0.5))
        .padding(3)
        .overlay(Circle().stroke(selection == tint ? primary @ 75 % : .clear, 1.5))
        .contentShape(Circle())
} .buttonStyle(.plain) .help(tint.label) } }
```

**PillPreview** (`SettingsView.swift:246-277`) — the animated miniature in `colors`:

```
TimelineView(.animation(minimumInterval: 1/30))
  HStack(2.5) { 13 × Capsule(color).frame(width: 3, height: h(i, t)) } .frame(height: 18)
.shadow(color @ 50 %, radius 4)
.padding(horizontal 12, vertical 8)
.background(Capsule().fill(black @ 88 %).overlay(Capsule().stroke(white @ 14 %, 0.75)))
```
with `thinking` → `4 + 10 * ((sin(6t − 0.55i) + 1)/2)` and listening →
`3 + 15 * (0.5 + 0.5·sin(3.1t + 0.9i)·sin(1.7t + 0.4i))` (`SettingsView.swift:269-276`).

**APIKeyRow** (`SettingsView.swift:421-456`): `LabeledContent(api_key)` containing either
`Label(saved, "checkmark.circle.fill")` in `plumeDone` + `Button(change_3)` (when a key exists), or
`SecureField(paste_key).frame(minWidth: 220)` + `Button(save).disabled(draft empty)`.

**UpdateStatus** (`SettingsView.swift:459-489`), a `HStack(8)` whose content depends on state:
`ProgressView().controlSize(.small)` + `checking` / `plume_is_up_to_date` + `Button(check_now)` /
`version_version_available` in `plumeTheme` + `Button(update_2)` / spinner + `installing` /
`Text(why)` `.orange` `.lineLimit(2)` + `Button(check_now)` / `Button(check_now)` alone.

**MicrophonePicker** (`SettingsView.swift:494-509`): `Picker(microphone)` with
`Text(macos_default).tag("")`, one entry per CoreAudio input, and a `disconnected_microphone`
entry kept visible when the stored UID is gone.

### 2.8 Onboarding (`Onboarding.swift`)

Window **560 × 540**, transparent title bar, hidden title, movable by background
(`Onboarding.swift:33-42`). Content:

```
VStack(spacing: 0)                                                    :92
├── ZStack { page(current) }                                          :93-101
│     .id(model.step)
│     .transition(.asymmetric(insertion: .move(edge: forward ? .trailing : .leading) + .opacity,
│                             removal:   .move(edge: forward ? .leading  : .trailing) + .opacity))
│     .frame(maxWidth: .infinity, maxHeight: .infinity).clipped()
└── footer                                                            :118-156
.frame(width: 560, height: 540)
.background(Color(nsColor: .windowBackgroundColor))
.id(prefs.appLanguage)      // changing language redraws the whole assistant
```

Step order (`Onboarding.swift:71-79`):

```
[.welcome, .microphone, .accessibility, .shortcut, .language, .model]
 + (Setup.localPolish != nil ? [] : [.cleanup])     // the Cleanup step disappears when local.json
 + [.done]                                          // already provides a server
```

So the count is **7 or 8** depending on `local.json`:

* **8 steps** (welcome, microphone, accessibility, shortcut, language, model, cleanup, done) when
  no local cleanup server is configured — the general case for a fresh install;
* **7 steps** when `local.json` already provides one (`Setup.localPolish != nil`), i.e. the
  `cleanup` step drops out. That is the case on the capture machine: the 7 dots visible in
  `assistant-1-welcome-en-clair.png` and `assistant-6-model-en-clair.png` show the 7-step variant,
  with `done` last. When `cleanup` is present it sits between `model` and `done`.

**Footer** (`Onboarding.swift:118-156`), identical on every step:

```
HStack
├── left, fixed 110 pt wide, leading:
│   step 0 → Button(skip) .buttonStyle(.plain) .secondary .help(use_the_default_settings)
│   else    → Button(back)  .buttonStyle(.plain) .secondary .disabled(step == 0) .opacity(step==0 ? 0 : 1)
├── Spacer()
├── HStack(spacing: 7): one Circle 7×7 per step,
│      fill = (i == step) ? plumeTheme : secondary.opacity(0.3)
├── Spacer()
└── right, fixed 110 pt wide, trailing:
    Button(step == .done ? get_started : continue)
      .buttonStyle(.borderedProminent) .controlSize(.large)
      .keyboardShortcut(.defaultAction)
.padding(.horizontal, 28).padding(.vertical, 22)
```

**Step page template** (`StepPage`, `Onboarding.swift:312-358`) — every step uses it:

```
VStack(spacing: 0)
├── Spacer(minLength: 30)
├── iconView .padding(.bottom, 22)
│     .app    → AppGlyph(size: 88)                       :344-345
│     .symbol → Image(systemName:) 34 pt medium, white
│                .frame(76 × 76)
│                .background(RoundedRectangle 20 continuous
│                    .fill(LinearGradient(#7C8CFF → #4338CA, topLeading → bottomTrailing)))
│                .shadow(#4338CA @ 30 %, radius 10, y 4)                     :346-356
├── Text(title) .system(size: 26, weight: .bold) .multilineTextAlignment(.center)  :324-326
├── Text(subtitle) .body .secondary .multilineTextAlignment(.center)
│                 .frame(maxWidth: 380) .padding(.top, 8)                    :327-332
├── content() .padding(.top, 28)
└── Spacer(minLength: 10)
.padding(.horizontal, 40).frame(maxWidth: .infinity, maxHeight: .infinity)
```

#### Step 1 — `welcome` (`Onboarding.swift:209-231`)

Icon: app icon (88 pt). Title `welcome_to_plume`, subtitle `voice_dictation_in_every_app_2`.
Content: a `Grid(alignment: .leading, horizontalSpacing: 14, verticalSpacing: 14)` with two rows:

| Row | Leading label | Control |
|---|---|---|
| 1 | `language` (`Text`, `.secondary`, `.gridColumnAlignment(.trailing)`) | segmented `Picker`, labels hidden, `.fixedSize()` |
| 2 | `appearance` (same style) | segmented `Picker` with `system` / `light` / `dark`, labels hidden |

Both pickers are `.focusable(false)` (`Onboarding.swift:220`, `:228`).

#### Step 2 — `microphone` (`Onboarding.swift:233-238`)

Icon `mic.fill`. Title `microphone`, subtitle `to_hear_your_voice_it_stays_on_your_mac`.
Content: `PermissionStatus(ok: mic == .authorized, action: allow_microphone)`.

`PermissionStatus` (`Onboarding.swift:361-379`): when ok, `Label(allowed, "checkmark.circle.fill")`
`.headline` in `plumeDone`; otherwise `Button(allow_microphone).controlSize(.large)`; the swap
animates with `.easeOut(0.2)`.

#### Step 3 — `accessibility` (`Onboarding.swift:240-246`)

Icon `accessibility`. Title `accessibility`, subtitle
`for_the_shortcut_and_text_insertion_turn_on_plume_in_the_list`. Content:
`PermissionStatus(ok: permissions.accessibility, action: open_settings)`.

#### Step 4 — `shortcut` (`Onboarding.swift:248-271`)

Icon `keyboard`. Title `shortcut`, subtitle `press_once_to_start_once_to_stop`.

```
VStack(spacing: 14)
├── Text(prefs.shortcut.label) 22 pt semibold rounded
│     .padding(horizontal 18, vertical 8)
│     .background(RoundedRectangle 10 .fill(primary @ 7 %))
│     .overlay(RoundedRectangle 10 .strokeBorder(primary @ 12 %))
├── ShortcutRecorder(shortcut: $prefs.shortcut, compact: true)
└── if the shortcut is the fn key (kind == .modifier && keyCode == 63):
      Text(prefs.shortcut.explanation) .callout .secondary .center .frame(maxWidth: 400)
      Button(open_keyboard_settings)
```

#### Step 5 — `language` (`Onboarding.swift:273-281`)

Icon `globe`. Title `spoken_language`, subtitle `the_language_you_dictate_in`. Content: a
**radio group** `Picker` (labels hidden) over `DictationLanguage.allCases`
(`automatic_detection_recommended`, `french`, `english`, `spanish`, `german`, `italian`,
`portuguese`).

#### Step 6 — `model` (`Onboarding.swift:283-288`, `402-453`)

Icon `cpu`. Title `voice_model`, subtitle
`the_one_recommended_for_this_mac_is_already_selected_you_can_change_it_in_models`.

```
VStack(spacing: 14)
├── Picker(labelsHidden, .fixedSize) over supported models            :412-419
│     label = "<displayName> · <size>" (+ " · recommended" / " · already here")
├── if cloud model chosen:
│     Form { CloudModelSection(showUseButton: false) }
│       .formStyle(.grouped).scrollDisabled(true).frame(460 × 210)    :420-424
└── else VStack(spacing: 6)
      ├── Text(current.summary)
      ├── Text(facts) .callout .secondary        // languages · speed · accuracy
      └── Text(this_mac_chip_memory_gb_of_memory) .caption .tertiary   :426-435
    .multilineTextAlignment(.center).frame(maxWidth: 420)
```

#### Step 7 (conditional) — `cleanup` (`Onboarding.swift:290-300`)

Icon `sparkles`. Title `text_cleanup`, subtitle
`fixes_punctuation_and_filler_words_optional_your_own_api_key`. Content:
`Form { PolishServiceRows(compact: true) }`, `.formStyle(.grouped)`, `.scrollDisabled(true)`,
frame **460 × 190**.

#### Last step — `done` (`Onboarding.swift:302-306`, `381-400`)

Icon: app icon. Title `you_re_all_set`, subtitle `press_shortcut_to_dictate`. Content:
`ModelDownloadStatus`, which is one of three states:

* progress available → `ProgressView(value: p).frame(width: 260)` + `Text(downloading_model_percent_of_size)`
  `.callout` `.secondary` in a `VStack(spacing: 8)`;
* error stored → `Text(error)` `.callout` `.red`;
* otherwise nothing (`EmptyView`).

"Skip" (`Onboarding.swift:172-190`) writes explicit defaults — language `en`, dictation language
`auto`, appearance `light`, pill style `bottom`, idle release `35` s — installs the recommended
model, requests microphone and accessibility, then finishes.

### 2.9 What's New (`WhatsNew.swift`)

Window **560 × 500**, same chrome as onboarding (`WhatsNew.swift:90-96`). Layout is the sibling of
onboarding (`WhatsNew.swift:122-164`):

```
VStack(spacing: 0)
├── ZStack { StepPage(icon: .symbol(page.symbol), title:, subtitle:) { EmptyView() } }
│     .transition(asymmetric move + opacity, direction-aware)                 :124-135
├── footer HStack: Button(back) 110 leading · Spacer · dots 7×7 (plumeTheme / secondary 30 %) ·
│     Spacer · Button(last ? got_it : next) .borderedProminent .large .defaultAction, 110 trailing
│     .padding(horizontal 28, vertical 22)                                    :137-156
.frame(560 × 500).background(windowBackgroundColor)
.overlay(alignment: .top) {
    Text(what_s_new_in_plume_version) .callout.semibold .secondary .padding(.top, 18)   :160-163
}
```

Page content comes from `ReleaseNotes.all` (`WhatsNew.swift:16-50`) — one page per change, filtered
to the versions crossed since the last run. Texts (hard-coded FR/EN pairs, not in the JSON):

| Version | Symbol | Title FR / EN | Text FR / EN |
|---|---|---|---|
| 1.5 | `textformat.alt` | Dictée 1337 / 1337 dictation | "Dictez normalement, puis terminez avec votre raccourci 1337 : ce texte part en leet speak. Un onglet 1337 permet aussi de convertir du texte déjà écrit." / "Dictate as usual, then finish with your 1337 shortcut: that text comes out in leetspeak. A 1337 tab also converts text you already have." |
| 1.5 | `rectangle.topthird.inset.filled` | Nouveaux styles de pastille / New indicator styles | "Trois emplacements : en bas au-dessus du Dock, en haut, ou en carte qui descend de l'encoche. Verre dépoli macOS, plus l'aplat noir." / "Three positions: bottom above the Dock, top, or a card sliding out of the notch. Real macOS material instead of a black slab." |
| 1.5 | `forward.end` | Passer la configuration / Skip the setup | "Un bouton Passer sur le premier écran : des réglages qui marchent tout de suite, sans parcourir sept étapes." / "A Skip button on the first screen: working settings straight away, without going through seven steps." |
| 1.4 | `text.append` | Commandes vocales / Voice commands | "Dites « à la ligne », « nouveau paragraphe », « ouvrez la parenthèse… fermez la parenthèse » ou « point d'interrogation ». Réglages → Dictée." / "Say “new line”, “new paragraph”, “open parenthesis… close parenthesis” or “question mark”. Settings → Dictation." |
| 1.4 | `mic` | Choix du micro / Choose your microphone | "Dictez avec le micro du Mac en gardant vos AirPods pour écouter. Réglages → Dictée → Micro." / "Dictate with the Mac's microphone while your AirPods stay on for listening. Settings → Dictation → Microphone." |
| 1.3 | `cpu` | Choisissez votre modèle / Choose your model | "Parakeet Ultra, Whisper jusqu'à Large v3 pour les Mac puissants, ou un service en ligne avec votre clé. Page Modèles." / "Parakeet Ultra, Whisper up to Large v3 for powerful Macs, or an online service with your own key. See Models." |
| 1.3 | `arrow.down.circle` | Mises à jour / Updates | "Plume se met à jour toute seule depuis GitHub. Vos réglages, votre historique et vos modèles sont conservés." / "Plume now updates itself from GitHub. Your settings, history and models are kept." |
| 1.3 | `menubar.rectangle` | Barre des menus / Menu bar | "Une icône facultative pour dicter, copier la dernière dictée ou ouvrir l'historique. Réglages → Général." / "An optional icon to dictate, copy your last dictation or open History. Settings → General." |
| 1.3 | `sparkles` | Nettoyage en anglais / Cleanup in English | "La consigne du nettoyage existe aussi en anglais, et suit la langue que vous parlez." / "The cleanup instructions now exist in English too, and follow the language you speak." |

---

## 3. The pill (`PillPanel.swift`)

The floating dictation indicator. It is a **borderless, non-activating `NSPanel`** that draws on
top of everything, takes no room in the menu bar, never becomes key or main, and never steals
focus (`PillPanel.swift:22-65`):

```
styleMask: [.borderless, .nonactivatingPanel]        PillPanel.swift:47
isFloatingPanel = true                               :49
level = .statusBar                                   :50
collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]   :51
backgroundColor = .clear, isOpaque = false, hasShadow = false      :52-54
hidesOnDeactivate = false, becomesKeyOnlyIfNeeded = true           :55-56
isMovable = false, animationBehavior = .none                       :57-58
canBecomeKey = false, canBecomeMain = false                        :64-65
```

The panel follows the app appearance, not the system one: `appearance = NSApp.appearance` on
every present (`PillPanel.swift:149`).

### 3.1 Geometry

| Constant | Value | Source |
|---|---|---|
| Regular pill card | **320 × 64 pt** (the window slot; the drawn capsule is intrinsic, ≈116 × 34) | `PillPanel.swift:26` |
| Popup card ("nothing to paste") | **430 × 96 pt** slot | `PillPanel.swift:28` |
| Shadow margin, horizontal | **26 pt** each side | `PillPanel.swift:88` |
| Shadow margin, bottom | **26 pt** | `PillPanel.swift:90` |
| Window size | `card.width + 52` × `card.height + 26` | `PillPanel.swift:98-99` |
| Bottom style resting position | `visibleFrame.minY + 14` (above the Dock) | `PillPanel.swift:122` |
| Top / notch style resting position | window top edge = `screen.frame.maxY` | `PillPanel.swift:125-126` |
| Horizontal position | centred on `visibleFrame.midX` | `PillPanel.swift:116` |

Why the window is larger than the card: the card's shadow needs room, otherwise the window edge
clips it into a hard rectangle (`PillPanel.swift:83-90`).

Drawn card sizes (measured on the captures, 2×):

| Variant | Card size | Evidence |
|---|---|---|
| Bottom/top pill, recording | **116 × 34 pt** | `pastille-bottom-sombre-ecoute.png` (134 × 58.5 incl. shadow) |
| Bottom/top popup | **430 × 56 pt**, one line of preview text | `pastille-bottom-clair-popup.png` (443 × 72 incl. shadow); the dark twin measures 428–433 pt wide |
| Bottom/top popup, two-line text | 430 × 73 pt (**computed**: second 14 pt line = +17) | — |
| Notch (for reference only, §3.9) | 276 wide (frame target), 60 pt tall recording, 74 pt popup | `pastille-notch-sombre-popup.png` measures exactly 276 × 74 |

Anchoring rules (`PillPanel.swift:113-128`):
* **bottom**: the **bottom edge is fixed** — the pill grows upward when the popup opens and never
  jumps; once visible, the current `frame.minY` is kept.
* **top** and **notch**: the **top edge is fixed** at the screen top — they grow downward.

### 3.2 Phases (`PillPhase`, `DictationController.swift:6-36`)

| Phase | What the pill shows | How long | Source |
|---|---|---|---|
| `.hidden` | nothing (the view is empty, the panel is ordered out) | — | `DictationController.swift:7` |
| `.recording` | 13 accent-coloured bars + ✕ button | until the user stops | `PillPanel.swift:384-386` |
| `.processing` | 13 amber bars (thinking wave), no ✕ | until the transcript is ready | `PillPanel.swift:387-388` |
| `.pasted` | a single ✓ in `plumeDone` | **0.9 s** | `DictationController.swift:402` |
| `.savedToHistory(text)` | **popup variant**: header + 2-line preview, stays until dismissed | **persistent** | `DictationController.swift:19-23`, `:407` |
| `.copied` | `Label(copied, "doc.on.clipboard.fill")` in `plumeDone` | **1.2 s** | `DictationController.swift:517` |
| `.message(text, isError)` | one label, icon `exclamationmark.triangle.fill` (orange) or `waveform.slash` (secondary) | **1.6 s** default (`flash`), **2.5 s** "microphone unavailable", **3 s** "engine unavailable" | `DictationController.swift:183`, `:237`, `:343` |

Two independent timers gate the gestures: `doubleTapWindow = 0.3 s`
(`DictationController.swift:72`) decides "cancel" vs "stop", and `maxRecordingSeconds = 600 s`
(`DictationController.swift:74`) force-stops a forgotten dictation.

Clicking the card (`PillPanel.swift:415`, `PillPanel.swift:448`):
* during `recording` / `processing` → opens the main window (`DictationController.swift:518-519`);
* during `savedToHistory` → copies the whole text and switches to `.copied`
  (`DictationController.swift:515-517`);
* otherwise nothing.

### 3.3 Surface, stroke and shadow — bottom & top

`pillBackground(capsule:)` (`PillPanel.swift:492-521`), for `.bottom` / `.top` only:

```
shape
  .fill(.ultraThinMaterial)
  .overlay(shape.fill(light: Color.white.opacity(0.30), dark: Color.black.opacity(0.34)))
  .overlay(shape.stroke(Color.white.opacity(light: 0.55, dark: 0.20), lineWidth: 0.75))
  .overlay(shape.stroke(Color.black.opacity(light: 0.10, dark: 0.28), lineWidth: 0.5).padding(0.5))
  .shadow(color: .black.opacity(light: 0.22, dark: 0.42), radius: 14, y: 5)
  .shadow(color: .black.opacity(0.10), radius: 2, y: 1)
```

Web equivalent:

```css
.pill {
  border-radius: 999px;                       /* capsule */
  backdrop-filter: blur(12px) saturate(180%);
  background: rgba(255,255,255,0.55);          /* dark: rgba(40,40,40,0.45) */
  box-shadow:
    0 5px 14px rgba(0,0,0,0.22),               /* dark: 0.42 */
    0 1px 2px rgba(0,0,0,0.10),
    inset 0 0 0 0.75px rgba(255,255,255,0.55); /* dark: 0.20 */
}
.pill::after {                                 /* inner hairline, 0.5 pt inset */
  content: ""; position: absolute; inset: 0.5px;
  border-radius: inherit;
  box-shadow: inset 0 0 0 0.5px rgba(0,0,0,0.10);  /* dark: 0.28 */
}
```

Text colour on the pill: `.primary` for bottom/top — i.e. `rgba(0,0,0,0.85)` light /
`rgba(255,255,255,0.85)` dark (`PillPanel.swift:548`). The notch card forces white.

### 3.4 Shape

* Capsule for the pill state: `Capsule()` (`PillPanel.swift:538`).
* `RoundedRectangle(cornerRadius: 16, style: .continuous)` for the popup state
  (`PillPanel.swift:540`).

### 3.5 Layout inside the pill (recording / processing / pasted / copied / message)

```
ZStack(alignment: .bottom)                        // bottom/top styles
├── Color.clear
└── content                                        (only when phase != .hidden)
    HStack(spacing: 8)                                                 :382
    ├── bars(accent, thinking:)   13 capsules                          :384-388
    └── when recording: ✕ button                                       :386, 550-562
    .frame(maxWidth: nil)                                              :407
    .padding(.horizontal, 12)                                          :408
    .padding(.vertical, 8)                                             :409
    .background(pillBackground(capsule: true))                          :413
    .contentShape(Capsule())
    .onTapGesture { pillClicked() }                                     :415
  .transition(hangsFromTop ? .identity : .scale(0.6, anchor: .bottom) + .opacity)  :345-347
  .padding(.horizontal, 26)                                             :352
  .padding(.top or .bottom, 43 (top style) / 6 (bottom style))          :353, 317
  .padding(.bottom, 26)  // only when it hangs from the top             :354
.animation(.spring(response: 0.32, dampingFraction: 0.78), value: phase) :358
```

Numbers for the 116 × 34 card: 13 bars × 3 pt + 12 gaps × 2.5 pt = **69 pt**; + 8 pt gap + 15 pt ✕
= 92; + 2 × 12 pt padding = **116**; height 18 pt bars + 2 × 8 = **34**.

`topInset` = **43 pt** for the top style (37 pt menu bar + 6), **0** for notch
(`PillPanel.swift:317`); the bottom style uses **6 pt** (`PillPanel.swift:353`).

### 3.6 The bars

`bars(color:thinking:)` (`PillPanel.swift:564-586`):

```
TimelineView(.animation(minimumInterval: 1/30)) { t in
    HStack(alignment: .center, spacing: spacing) {
        ForEach(0..<13) { i in Capsule().fill(color).frame(width: 3, height: height(i, t)) }
    }.frame(height: 18)
}.shadow(color: color.opacity(0.5), radius: 4)
```

13 bars: `LevelMeter.barCount = 13` (`DictationController.swift:41`). Bottom/top sizes: bar
width **3 pt**, gap **2.5 pt**, container height **18 pt** (`PillPanel.swift:570-572`) — that is
the *frame*; an individual bar can grow past it (up to 22 pt on a loud input) and overflow
visibly. Total bar strip: 13 × 3 + 12 × 2.5 = **69 pt**.

Height formulas (`PillPanel.swift:591-604`), `i` = bar index 0…12, `t` = seconds since reference
date:

* **processing (thinking wave)**
  `h = 4 + 10 * (sin(6t − 0.55·i) + 1) / 2`  → range 4…14 pt.
* **recording (live level + shimmer)**
  `level = meter.levels[i]` (0…1, most recent sample on the right)
  `shimmer = 0.5 + 0.5 * sin(7t + 1.3·i)`
  `h = max(3, 3 + 15 * level^0.8 + 4 * shimmer)`  → range 3…22 pt.
  The `max(3, …)` floor plus the shimmer guarantees the pill never shows thirteen aligned dots
  when no audio arrives (`PillPanel.swift:597-604`).
* Bars are **not** rounded rectangles but capsules, so each end is a semicircle.

Bar colour: `.plumeAccent` = the listening tint, default `#33D6E0`
(`PillPanel.swift:385`, `Prefs.swift:421`); `.plumeThinking` = the processing tint, default
`#F5A623` (`PillPanel.swift:388`, `Prefs.swift:423`).

### 3.7 Other states, exact drawing

| State | Drawing | Source |
|---|---|---|
| `pasted` | `Image("checkmark")` 13 pt bold, `plumeDone`, `.frame(width: 30, height: 18)` | `PillPanel.swift:389-393` |
| `copied` | `Label(copied, "doc.on.clipboard.fill")` with `PillLabelStyle(color: plumeDone)` — icon 12 pt in the given colour, title 12 pt medium in `.primary` @ 88 %, `HStack(spacing: 6)`, `.fixedSize()` | `PillPanel.swift:394-396`, `:607-617` |
| `message` | same label style; icon `exclamationmark.triangle.fill` when `isError` else `waveform.slash`; icon colour `.orange` or `.secondary`; text `.primary` @ 88 % | `PillPanel.swift:397-399` |
| ✕ cancel (pill) | `Image("xmark")` 7 pt **bold**, `pillText` @ 62 %, 15 × 15 frame, `Circle` filled `pillText` @ 10 %, `.buttonStyle(.plain)`, `.help(cancel)` | `PillPanel.swift:550-562` |

### 3.8 Popup variant (`savedToHistory`)

Triggered when there was **no text field** to paste into; the text is kept in history, the pill
grows and offers to copy it. `PillPhase.showsPreview` selects the 430 × 96 slot
(`DictationController.swift:26-29`, `PillPanel.swift:95-97`).

```
savedPreview(text)                                                   :422-450
VStack(alignment: .leading, spacing: 6)
├── HStack(spacing: 6)
│   ├── Image("tray.and.arrow.down.fill") 11 pt semibold, .plumeAccent                :425-427
│   ├── Text(no_text_field_click_to_copy) 11 pt medium, pillText @ 62 %                :428-430
│   ├── Spacer(minLength: 0)
│   └── ✕ close button: Image("xmark") 7 pt heavy, pillText @ 55 %,
│         14 × 14 frame, Circle filled pillText @ 10 %, .buttonStyle(.plain), .help("Close")  :459-471
├── Text(preview) 14 pt, pillText, .lineLimit(2), leading, .fixedSize(v: true)         :434-440
.frame(maxWidth: nil, alignment: .leading)                                            :442
.padding(.horizontal, 14).padding(.vertical, 10)                                      :443-444
.background(pillBackground(capsule: false))   // RoundedRectangle 16                    :446
.contentShape(RoundedRectangle(cornerRadius: 16, style: .continuous))                 :447
.onTapGesture { pillClicked() }  .help(copy_text)                                     :448-449
```

The preview is the **first five words** of the text followed by `…`; the whole text is copied
(`PillPanel.swift:453-457`):

```swift
let words = text.split(whereSeparator: \.isWhitespace)
guard words.count > 5 else { return text }
return words.prefix(5).joined(separator: " ") + "…"
```

Sample from the capture (`pastille-bottom-clair-popup.png`): header "No text field · click to
copy", body "Je voulais te dire que…" — 14 pt, two lines max, left-aligned, 14 pt side padding,
10 pt top/bottom.

### 3.9 Notch style (out of scope for the web port, kept for completeness)

The order asks to ignore the notch style; the numbers are listed here only so nothing is lost.
The card is **flush with the screen top**, bottom corners **14 pt continuous**, top corners 0
(`PillPanel.swift:534-536`, `NotchCardShape` at `:626-651`), filled flat `#0E0E0E`
(`PillPanel.swift:372`) with **no stroke and no shadow** in the shipping view
(`PillPanel.swift:365-373`); the content variant adds `#0E0E0E` @ 97 % + a 0.75 pt white @ 13 %
stroke + black @ 50 % / radius 20 / y 9 shadow (`PillPanel.swift:504-506`). Bars are widened to
**7 pt** with **9 pt** gaps and **28 pt** max height (`PillPanel.swift:570-572`). Horizontal
padding is 22 pt and vertical 8 pt, with an extra 12 / 8 pt outer padding (`PillPanel.swift:327-329`,
`:408-409`), and `topInset` is 0 (`PillPanel.swift:317`). Entry/exit: the window itself slides from
`goal.maxY + 4` (`PillPanel.swift:184`), the view plays **no** transition
(`PillPanel.swift:345-347`).

### 3.10 Motion of the panel

| Action | Mechanism | Duration | Curve | Source |
|---|---|---|---|---|
| Appear, bottom style | no animation: `setFrame` + `alphaValue = 1` + `orderFrontRegardless`; the *view* plays `.scale(0.6, anchor: .bottom)` + opacity via the spring | spring 0.32 / 0.78 | spring | `PillPanel.swift:157-161`, `:345-347` |
| Appear, top/notch | the **window** is moved by a 60 Hz `Timer`, starting at `goal.maxY + 4` (fully above the screen), opaque from the first pixel | **0.34 s** | ease-out cubic `1−(1−t)³` | `PillPanel.swift:177-231` |
| Resize while visible (e.g. pill → popup) | `NSAnimationContext` + `animator().setFrame` | **0.22 s** | easeInEaseOut | `PillPanel.swift:164-170` |
| Re-place after a style change | same, without presenting | **0.28 s** | easeInEaseOut | `PillPanel.swift:135-143` |
| Disappear, bottom style | `orderOut` immediately | 0 | — | `PillPanel.swift:269-272` |
| Disappear, top/notch | window moves to `screen.frame.maxY + 10` (fully off-screen) then `orderOut`; the **content stays visible during the whole trip** | **0.22 s** | ease-in quadratic `t²` | `PillPanel.swift:236-281` |

The slide is done by moving the window, not by fading: a fade was explicitly rejected because it
read as "appearing" instead of "gliding" (`PillPanel.swift:178-182`, `:336-344`). For a CSS port:
`transform: translateY(-100%) → translateY(0)` with
`transition: transform 340ms cubic-bezier(0.33, 1, 0.68, 1)` (ease-out-cubic) in, and
`220ms cubic-bezier(0.32, 0, 0.67, 0)` (ease-in-quad) out.

Two duration sources disagree and the **code wins**: the comment above the entry animation says
"glisse en 0,42 s" (`PillPanel.swift:175-176`) while the call passes `0.34`
(`PillPanel.swift:203`). Use **0.34 s**. Similarly, the 37 pt menu-bar height used for `topInset`
is a constant in the view (`PillPanel.swift:313-317`), not read from `NSScreen`.

---

## 4. Menu bar item (`StatusItem.swift`)

Optional (`show_in_menu_bar`, off by default — `Prefs.swift:258-264`). The status item is
`NSStatusItem.squareLength` with a hand-drawn 18 × 18 pt template feather image
(`StatusItem.swift:15-16`, `:88-110`), accessibility label "Plume" (`StatusItem.swift:17`), and a
menu rebuilt on every open (`menuNeedsUpdate`, `StatusItem.swift:31-71`), `autoenablesItems = false`
(`StatusItem.swift:20`).

Menu items, **in order**:

1. **Dictation toggle** (`StatusItem.swift:36-48`)
   * processing → title `processing_2`, disabled, no action;
   * recording → title `stop_dictation`, icon `stop.circle`;
   * idle → title `start_dictation_2` + `"  (<shortcut label>)"` (two spaces before the
     parenthesis), icon `mic`.
2. **Copy last dictation** — `copy_last_dictation`, icon `doc.on.doc`, enabled only when a
   dictation exists (`StatusItem.swift:50-54`).
3. *separator*
4. **History…** — `history_2`, icon `clock.arrow.circlepath` (`StatusItem.swift:59`).
5. **Settings…** — `settings`, icon `gearshape` (`StatusItem.swift:60`).
6. **Support Plume…** — `support_plume`, icon `cup.and.saucer`; opens
   `https://buymeacoffee.com/ppocass` (`StatusItem.swift:61`, `Support.swift:12`).
7. *separator*
8. **Quit Plume** — `quit_plume`, icon `xmark.rectangle` (`StatusItem.swift:68-70`).

## 5. Application menus (`AppDelegate.swift:243-286`)

For completeness (a web port usually drops these, but they carry UI strings):

* **Plume**: `about_plume` · `check_for_updates` (only when the updater is supported) ·
  `report_a_problem` · — · `settings` (⌘,) · — · `hide_plume` (⌘H) · — · `quit_plume` (⌘Q).
* **Edit**: `undo` (⌘Z) · `redo` (⇧⌘Z) · — · `cut` (⌘X) · `copy` (⌘C) · `paste` (⌘V) ·
  `select_all` (⌘A) · — · `search_history` (⌘F).
* **Window**: `close` (⌘W) · `minimize` (⌘M).

---

## 6. Localisation files

`docs/strings/en.json` and `docs/strings/fr.json`: 304 flat `key → string` entries, identical key
sets, UTF-8, two-space indented, keys sorted by first appearance in the sources.

### 6.1 Where the strings come from, and what a "key" is

The macOS app has **no string table**: every string is written inline as
`L("français", "English")` (`L10n.swift:37`), a function that returns one of its two arguments.
There are therefore **no Swift identifiers to use as keys**. The rule used here, applied
mechanically to every `L(...)` call site in `*.swift` plus the `L(fr, en)` payloads held in
`Shortcut.swift:62-72` and `ModelCatalog.swift:109-182`:

```
key = lowercase(English text)
      with {placeholder} kept as a word,
      non-alphanumerics collapsed to "_",
      keyboard symbols spelled out:  ⌥→opt  ⌘→cmd  ⌃→ctrl  ⇧→shift
      on collision (same English, different French): suffix "_2", "_3", …
```

Examples: `"Press \(shortcut) to start."` → `press_shortcut_to_start`;
`"Left ⌥"` → `left_opt`; `"Settings…"` → `settings`, `"Settings"` → `settings_2`.

The 12 collision groups (same English wording, different French) are: `settings`/`settings_2`,
`edit`/`edit_2`, `fix_a_word`/`fix_a_word_2`, `history`/`history_2`,
`voice_dictation_in_every_app`/`_2`, `start_dictation`/`start_dictation_2`, `light`/`light_2`
("Clair" vs "Léger"), `processing`/`processing_2`, `change`/`change_2`/`change_3`,
`update`/`update_2`, `heard`/`heard_2`.

### 6.2 Placeholders

Swift interpolation `\(expr)` became `{name}`; the French and English values of a key always carry
the **same** placeholder names. The 29 keys with placeholders:

| Key | Placeholder | Meaning |
|---|---|---|
| `press_shortcut_to_start`, `press_shortcut_to_dictate` | `{shortcut}` | current shortcut label, e.g. "⌥ gauche" / "Left ⌥" |
| `count_dictations_will_be_deleted`, `count_words` | `{count}` | integer count (words, or stored dictations) |
| `couldn_t_transcribe_seconds_s_of_audio_kept` | `{seconds}` | rounded audio duration in seconds |
| `downloading_percent` | `{percent}` | download progress 0…100 |
| `downloading_model_percent_of_size` | `{model}`, `{percent}`, `{size}` | model display name, progress, human size |
| `name_idle`, `name_loading`, `name_ready`, `name_ready_shared`, `name_error`, `name_is_not_downloaded` | `{name}` | active model display name |
| `error_code_on_file`, `corrupted_file_file` | `{code}`, `{file}` | HTTP/URL error code, file name |
| `delete_model` | `{model}` | model name |
| `size_will_be_freed_you_can_download_it_again` | `{size}` | human-readable size |
| `speed_speed`, `accuracy_accuracy` | `{speed}`, `{accuracy}` | one of slow/fast/very fast, good/very good/excellent |
| `memory_gb_memory_recommended` | `{memory}` | GB of RAM |
| `requires_macos_version` | `{version}` | minimum macOS major version |
| `create_a_service_key` | `{service}` | service label |
| `this_mac_chip_memory_gb_of_memory` | `{chip}`, `{memory}` | chip name, GB |
| `version_version_available`, `plume_version_is_available`, `what_s_new_in_plume_version`, `you_have_version_version_your_settings_history_and_models_are_kept` | `{version}` | version string |
| `what_not_found` | `{what}` | missing binary/shortcut name |

### 6.3 Strings that are *not* in the JSON

* Release-note titles and bodies (`WhatsNew.swift:17-50`) — they go through `L(page.titleFR,
  page.titleEN)` with stored properties, so they have no literal call site. They are reproduced
  verbatim in §2.9.
* `local.json`-provided notes (`Setup.swift:109`, `Setup.swift:118`) — user-supplied text.
* Pure data: model names ("Parakeet Ultra", "Whisper Turbo"…), format units from
  `L10n.swift:26-34` (`octets/Ko/Mo/Go` vs `bytes/KB/MB/GB`), the hard-coded sidebar title `1337`
  (`MainWindow.swift:77`) and the app name `Plume`.

### 6.4 A note on `%.2f`

`ok_2f_s` is `"OK · %.2f s"` in both languages (`SettingsView.swift:204`) — a `String(format:)`
pattern, not a `{name}` placeholder. Keep it as a printf template and substitute the latency in
seconds.

---

## 7. Quick reference — the 25 numbers to hard-code first

| What | Value | Source |
|---|---|---|
| Main window | 860 × 580, min 680 × 440 | `MainWindow.swift:42`, `:48` |
| Sidebar widths | 190 / 210 / 280 | `MainWindow.swift:107` |
| Home content column | 720 max, 28 pt padding, 22 pt block spacing | `HomeView.swift:56-58`, `:12` |
| Card radius / fill | 12 pt continuous / `quaternary` @ 50 % | `HomeView.swift:52`, `:83`, `:140` |
| Accent | `#4F46E5` light, `#8B93FF` dark | `Prefs.swift:417-418` |
| Done green | `#5FD29A` | `Prefs.swift:425` |
| Default listening / processing tints | `#33D6E0` / `#F5A623` | `Prefs.swift:73`, `:80` |
| Window background | `#FFFFFF` / `#1E1E1E` | measured |
| Pill card (drawn) | 116 × 34, capsule | §3.1 |
| Pill window slot | 320 × 64 (+26 h, +26 bottom margins) | `PillPanel.swift:26`, `:88-90` |
| Pill top inset | 43 pt (37 menu bar + 6) | `PillPanel.swift:317` |
| Pill popup | 430 × 96 slot, 16 pt radius | `PillPanel.swift:28`, `:540` |
| Pill bars | 13 × 3 pt, 2.5 pt gap, 18 pt max | `DictationController.swift:41`, `PillPanel.swift:570-572` |
| Pill in / out | 340 ms ease-out-cubic / 220 ms ease-in-quad | `PillPanel.swift:203`, `:243` |
| Pill view spring | response 0.32, damping 0.78 | `PillPanel.swift:358` |
| Onboarding window | 560 × 540, 7 or 8 steps (see §2.8) | `Onboarding.swift:33`, `:71-79` |
| Onboarding icon tile | 76 × 76, radius 20, gradient `#7C8CFF → #4338CA` | `Onboarding.swift:350-355` |
| Onboarding page spring | 0.42 / 0.88 | `Onboarding.swift:201` |
| What's New window | 560 × 500 | `WhatsNew.swift:90` |
| Prominent button | capsule, 28 pt tall (`.large`) | measured |
| History row | 12 pt gap, 6 pt vertical padding, 10 pt tags | `HistoryView.swift:26`, `:96`, `:202` |
| Vocabulary sheet | 440 × 440 | `VocabularyView.swift:190` |
| Fix-a-Word sheet | 420 wide, 20 pt padding | `VocabularyView.swift:245-246` |
| Sidebar footer | 12 pt text, 7 pt dot, 16/11 pt padding | `MainWindow.swift:264-275` |
| Form / list metrics | platform grouped-form and `.inset` list metrics, never overridden | system |

---

*End of specification. Everything above is derived from the listed source lines; where a value is
a macOS system metric or a measurement, it is labelled as such.*
