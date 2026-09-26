---
name: wisp
description: Ghost-text command suggestions for zsh from a small model that runs on your Mac.
colors:
  ink: "#0a0a18"
  ink-deep: "#07070f"
  bone: "#f3ecdc"
  bone-dim: "#b9b3a6"
  ash: "#8a8578"
  ghost-edge: "#4d4650"
  cathode: "#ff6a1a"
  bronze: "#8a6a3a"
typography:
  display:
    fontFamily: "Barlow Condensed, Arial Narrow, sans-serif"
    fontSize: "clamp(5rem, 11vw, 12.5rem)"
    fontWeight: 400
    lineHeight: 1
  headline:
    fontFamily: "Barlow Condensed, Arial Narrow, sans-serif"
    fontSize: "clamp(2.5rem, 5.2vw, 5rem)"
    fontWeight: 500
    lineHeight: 1
    letterSpacing: "0.01em"
  command:
    fontFamily: "M PLUS 1 Code, ui-monospace, SF Mono, Menlo, monospace"
    fontSize: "max(calc(50 * 100vw / 1536), 1.5rem)"
    fontWeight: 500
    lineHeight: 1.1
    letterSpacing: "-0.04em"
  body:
    fontFamily: "M PLUS 1, system-ui, sans-serif"
    fontSize: "1.0625rem"
    fontWeight: 400
    lineHeight: 1.6
    fontFeature: "tnum"
  label:
    fontFamily: "M PLUS 1 Code, ui-monospace, SF Mono, Menlo, monospace"
    fontSize: "0.95rem"
    fontWeight: 400
    lineHeight: 1.6
rounded:
  focus: "2px"
  keycap: "4px"
  pill: "999px"
spacing:
  gutter: "calc(66 * 100vw / 1536)"
  gutter-narrow: "1.25rem"
  section: "clamp(6rem, 14vw, 12rem)"
  column-gap: "clamp(2rem, 6vw, 6rem)"
  heading-gap: "1.5rem"
  container: "1440px"
components:
  button-pill:
    backgroundColor: "transparent"
    textColor: "{colors.cathode}"
    rounded: "{rounded.pill}"
    padding: "0.74em 1.3em"
  button-pill-hover:
    backgroundColor: "{colors.cathode}"
    textColor: "{colors.ink}"
  button-copy:
    backgroundColor: "transparent"
    textColor: "{colors.bone-dim}"
    rounded: "{rounded.pill}"
    padding: "0.3em 0.9em"
  button-copy-hover:
    textColor: "{colors.cathode}"
  tab-scenario-selected:
    backgroundColor: "transparent"
    textColor: "{colors.cathode}"
    padding: "0.25rem 0"
  keycap:
    backgroundColor: "transparent"
    textColor: "{colors.bone}"
    rounded: "{rounded.keycap}"
    padding: "0.05em 0.45em"
  nav-link:
    textColor: "{colors.bone}"
  nav-link-hover:
    textColor: "{colors.cathode}"
---

# Design System: wisp

## Overview

**Creative North Star: "Struck Cathode Gauze"**

wisp is drawn as a single dark instrument plane: an ink-blue black field that runs past every edge, with an undulating bronze wire gauze plate behind the first viewport and again behind the measured figures. Type sits on the gauze with no bar, frame or window chrome. The one event in the world is a strike: of all the candidates, one name lights in cathode orange with a tube halo while the rest stay unlit, outlined ghosts. Everything else is bone-white type and space.

Density is low and the rhythm is long. Sections are separated by large vertical space, not by lines or containers, and each section carries one heading in condensed display type, some prose in M PLUS 1, and whatever real command text it needs in M PLUS 1 Code. Orange is reserved for what wisp picks or what you can act on, so it reads as signal, never decoration.

The world refuses the terminal-window mockup and the feature-card grid: terminal output is set as bare monospace on the plane, and grouping comes from depth (the gauze) and space.

**Key Characteristics:**
- One ink plane, full bleed; the bronze gauze plate is the only imagery and the only texture.
- Two hues: ink blue and cathode orange. Neutrals are warm bone and ash.
- Lit versus unlit: the chosen value glows in cathode, alternatives stay as hollow ghost outlines in register.
- Three typefaces with fixed jobs: condensed display, code mono, humanist sans prose.
- The only control shape is the outlined pill.

## Colors

A near-black ink field, warm bone type, and one incandescent orange that means "picked" or "press this".

### Primary
- **Cathode Orange** (cathode): the struck completion, lit numerals, step numbers, the selected scenario tab, the completion in the demo prompt, highlighted source names in terminal output, pill outlines and hover states, focus rings and text selection. When it is lit type it carries the tube halo (see Elevation & Depth).

### Secondary
- **Gauze Bronze** (bronze): the hairline accent of quiet controls: the copy-button outline, keycap borders, and the scrollbar thumb. It is the color of the wire gauze plate, so it reads as part of the material rather than as a second accent.

### Neutral
- **Tube Ink** (ink): the page plane, the pill hover text, the theme color and favicon field.
- **Deep Ink** (ink-deep): the scrollbar track, the only place the plane goes darker.
- **Bone** (bone): headings, the wordmark, nav, hero command and subline, inline code, and emphasized words in lists.
- **Dim Bone** (bone-dim): paragraphs, readout labels and notes, list body text, the unit after a numeral, copy-button text at rest.
- **Ash** (ash): the quietest legible text: provenance footnotes and the colophon (about 5.3:1 on ink).
- **Ghost Edge** (ghost-edge): the outline of an unlit numeral sitting behind a lit one, and the `$ ` sigil before copyable commands. It is a stroke and ornament color, not a text color (about 2.2:1 on ink).

### Named Rules
**The Two Hues Rule.** The page has ink blue and cathode orange, nothing else. Bone, ash and bronze are warm neutrals of the gauze; no third hue enters.

**The Strike Rule.** Cathode marks the one thing wisp picked or the one thing you can do. If two unrelated things on a screen glow, one of them is wrong.

## Typography

**Display Font:** Barlow Condensed 400/500 (with Arial Narrow, sans-serif)
**Body Font:** M PLUS 1 (with system-ui, sans-serif)
**Label/Mono Font:** M PLUS 1 Code (with ui-monospace, SF Mono, Menlo, monospace)

**Character:** A tall condensed grotesque for headings and tube numerals against a round, open Japanese-designed sans and its matching code mono. The mono is not a costume: every mono string on the page is a real command, a real completion, or a label on a measurement.

### Hierarchy
- **Display** (Barlow Condensed 400, clamp(5rem, 11vw, 12.5rem), line-height 1): tube numerals in the measured readouts. Step numbers use the same face at 3.25rem (2.5rem on phones) in cathode.
- **Headline** (Barlow Condensed 500, clamp(2.5rem, 5.2vw, 5rem), line-height 1, 0.01em): one per section, balanced wrap. The hero headline is the same face at a comp-scaled max(47 comp-px, 2.1rem), line-height 1.12, 3.4rem on phones.
- **Command** (M PLUS 1 Code 500, max(50 comp-px, 1.5rem), line-height 1.1, -0.04em): the hero command line. The struck completion is 500 at 37 comp-px with 0.015em tracking; ghost candidates are 300 at 36 comp-px. The demo prompt line runs clamp(1.1rem, 2.2vw, 1.9rem).
- **Body** (M PLUS 1 400, 1.0625rem, line-height 1.6, tabular numerals): prose capped at 66ch in bone-dim; lead paragraphs and list items step to 1.125rem.
- **Label** (M PLUS 1 Code 400, 0.95rem): readout terms and scenario tabs (1rem). Footnotes drop to 0.875rem in the sans; the colophon is mono at 0.9rem.

The wordmark is "wisp", always lowercase, set in M PLUS 1 500 (max(36 comp-px, 1.5rem) in the masthead, 1.4rem in the colophon). There is no drawn logo.

### Named Rules
**The Real Strings Rule.** Mono is only for text a terminal would print or a user would type. Prose never goes mono for flavor.

**The Condensed Heads Rule.** Barlow Condensed carries headings and numerals only, never paragraphs or controls.

## Layout

The hero is measured against a 1536×1024 comp: a comp unit (`--u`, 100vw / 1536; 100vw / 390 at 760px and below) scales the hero's type and positions with the viewport. On desktop the command line and pitch sit absolutely at their measured places (line at 16.2% / 36.8%, pitch at 4.5% / 67.5%, 60% wide); the candidate stack hangs from the caret. At 760px and below the hero becomes a bottom-aligned column: command, stack, then pitch, gap 3rem.

Below the hero, sections share the plane: `section` top padding, horizontal gutter (never under 1.25rem), max width 1440px, centered. Two-column groups use a 5fr / 7fr split for copy and demo, and equal halves for readouts and the local-first copy, with column-gap. Readouts collapse to one column at 1100px; every grid collapses at 760px. Section headings sit heading-gap above their content. Install steps are a two-column grid (3.5rem numeral column, 2.5rem on phones) with 2.5rem between steps. The colophon closes the page as one wrapped mono row.

**The Space Not Boxes Rule.** Groups are separated by section space and column gap. There are no cards, panels, dividers or horizontal rules.

## Elevation & Depth

The system is flat; depth is optical, not stacked. Two devices carry it. The **gauze plate** (a bronze wire mesh photograph, saturate 0.85) sits behind the hero, masked to fade into ink at every edge, and returns as a band at 0.55 opacity behind the figures so the numerals hang in the same plane as the hero. The **tube halo** is a four-layer zero-offset orange text-shadow that makes lit type read as an incandescent filament. It is the world's native light source, drawn only on text that wisp has struck (the hero completion, lit numerals, the demo completion); highlighted names inside terminal output get a softer single-layer glow. There are no box shadows.

### Shadow Vocabulary
- **Tube halo** (`text-shadow: 0 0 0.12em rgb(255 106 26 / 0.9), 0 0 0.5em rgb(255 106 26 / 0.75), 0 0 1.2em rgb(255 106 26 / 0.45), 0 0 2.4em rgb(255 106 26 / 0.22)`): struck completions and lit numerals.
- **Source glint** (`text-shadow: 0 0 0.6em rgb(255 106 26 / 0.45)`): the source name marked inside terminal output.

### Named Rules
**The Filament Rule.** Glow is emitted by lit cathode text and nothing else. No glowing boxes, buttons, borders or backgrounds.

## Shapes

Almost nothing on the page has a silhouette. The only outlined shapes are the fully rounded pill (999px) for controls and the small keycap (4px) for keyboard keys, both drawn as outlines on the ink with no fill at rest. Focus rings are a 2px cathode outline at 4px offset with a 2px corner. Unlit values are drawn as hollow type: a 1–1.5px text stroke with no fill.

## Components

### Buttons
Outlined, rounded, mono-set; they fill only when you reach for them.
- **Shape:** fully rounded pill (999px).
- **Primary pill:** 2px cathode outline, cathode mono 500 text, padding 0.74em 1.3em, optional inline GitHub mark at 1.15em. Used for "Install from GitHub" and "Read the source on GitHub".
- **Hover / Focus:** fills cathode with ink text over 160ms ease-out; focus is the global cathode ring.
- **Copy pill:** 1px bronze outline, bone-dim mono at 0.85rem, padding 0.3em 0.9em. Hover and the copied state turn text and outline cathode over 160ms ease-out.

### Tabs
- **Scenario tabs:** bare mono words in a wrapping row (gap 0.25rem 1.5rem), no underline or box. The selected tab is cathode; hover lifts an idle tab to bone-dim.

### Navigation
- **Masthead:** no bar. Lowercase wordmark top left, one "GitHub" link top right, both bone M PLUS 1 500, sitting directly on the gauze. Links turn cathode on hover.

### Keycaps
- **Style:** 1px bronze outline, 4px corners, bone mono at 0.85em, padding 0.05em 0.45em, inline in prose for →, End, Ctrl and F.

### Command Row
- **Style:** a bone mono command preceded by a ghost-edge `$ ` sigil, wrapping anywhere, with a copy pill aligned to its baseline (stacked under it on phones). No surrounding box.

### Candidate Stack (signature)
The hero's command line with its candidates hanging from the caret: a 2px bone caret, unlit candidates as hollow mono (fill #10162a, 1px stroke #3a4a64, weight 300), then the struck completion in cathode with the tube halo. The struck value snaps in, never tweens; the previous value decays over one beat (700ms, cubic-bezier(0.16, 1, 0.3, 1), opacity 0.55 to 0 with a 3px blur) while the next strikes. Real completions cycle every 3.4s after a 5.2s hold. Reduced motion removes the decay.

### Tube Readout (signature)
A measured value as a tube numeral: the lit value in cathode display type with the halo, the unit or denominator at 0.42em in bone-dim, and when a before-value exists, the unlit numeral sits in register behind it as a ghost-edge outline (1.5px stroke, no fill). A mono label above and a bone-dim note (max 44ch) below.

## Do's and Don'ts

### Do:
- **Do** keep the page on one ink plane (#0a0a18) running to every edge, with the gauze plate masked into it.
- **Do** reserve cathode for the struck value and for controls, and give struck text the four-layer tube halo.
- **Do** show alternatives as hollow ghost outlines in register behind or beside the lit value.
- **Do** set every command, completion and terminal line in M PLUS 1 Code, and only those.
- **Do** make controls outlined pills: cathode 2px for the primary action, bronze 1px for quiet ones.
- **Do** separate sections with the section space (clamp(6rem, 14vw, 12rem)) and let prose wrap at 66ch.
- **Do** strike, don't tween: value changes snap, and only the outgoing glow decays over one beat.

### Don't:
- **Don't** draw terminal windows, title bars, traffic-light dots or code panels; terminal text sits bare on the plane.
- **Don't** group content in cards, bordered panels, dividers or horizontal rules.
- **Don't** put glow or shadow on boxes, buttons or borders; the halo belongs to lit cathode text only.
- **Don't** introduce a third hue, a gradient fill, or a second accent beside cathode.
- **Don't** set prose or controls in Barlow Condensed, or prose in mono.
- **Don't** use ghost-edge (#4d4650) for readable text; it is for outlines and the `$ ` sigil.
- **Don't** capitalize the wordmark; it is always "wisp".
