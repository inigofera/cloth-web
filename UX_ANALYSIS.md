# UX Analysis — Top 3 Problems

**Project:** cloth-web (personal wardrobe management, outfit logging & analytics)
**Date:** 2026-11-04
**Scope:** Full frontend (`frontend/src`), backend route behavior (`src/`) and data contract (`README.md`, `REPO_DESCRIPTION.md`)
**Method:** Code review of all five primary views (Clothes, Outfits, Insights, Tables, Settings), the shared wardrobe store, API client, routing layer, error handling, and the Rust route handlers that define what data actually reaches the UI.

---

## Executive Summary

| # | Problem | One-line summary | Severity |
|---|---------|------------------|----------|
| 1 | Silent data truncation with contradictory cap messaging | The backend caps every table read at 500 rows, the UI warns about a "100-row" cap that doesn't exist, and analytics are computed over partial data without saying so. | High — erodes trust in every number the app shows |
| 2 | The daily logging loop is not the most obvious feature | Logging "today's outfit" — the habit-forming core action — requires extra steps, gives no visible feedback on mobile, and offers no fast path for its highest-frequency use case. | High — friction compounds daily; kills retention of the core habit |
| 3 | Custom interactive controls lack semantics (accessibility) | Clickable divs, mouse-only drag-and-drop, and unmanaged dropdowns mean keyboard and screen-reader users cannot log outfits or arrange their dashboard at all. | High — entire flows are unusable for non-mouse users; systemic pattern risk |

Secondary concerns (watchlist) are covered in §4.

---

## Status & Implementation Log

Implementation status for the fixes proposed below. Updated as work lands; each entry links to a log section at the end of this document.

### Task status

| ID | Task | Problem | Status |
|----|------|---------|--------|
| P1-A1 | Honest cap-banner copy (no false numbers) | 1 | ✅ done (Step 5) |
| P1-B1 | Backend: `X-Total-Count` header + `?limit=` on `/db/:table`, count on `/outfits` | 1 | ✅ done (Step 4) |
| P1-B2 | Frontend client reads counts; store tracks totals / `truncated` | 1 | ✅ done (Step 5) |
| P1-B3 | Per-view truncation notices (Clothes, Outfits, Insights) with real numbers | 1 | ✅ done (Step 5) |
| P2-A1 | Preselect today by default in Outfits view | 2 | ✅ done (Step 1) |
| P2-A2 | "Log today's outfit" quick action | 2 | ✅ done (Step 1) |
| P2-A3 | Day selection reveals panel on mobile (scroll + focus) | 2 | ✅ done (Step 1) |
| P2-A4 | Picker recency ordering (+ last-worn display, pure helper + tests) | 2 | ✅ done (Step 1) |
| P2-A5 | Try-on generation: elapsed-time progress + cancel (AbortController) | 2 | ✅ done (Step 1) |
| P2-B1 | First-run onboarding card with cross-linked CTAs | 2 | ✅ done (Step 1) |
| P3-1 | Calendar ARIA grid: button cells, labels, keyboard navigation | 3 | ✅ done (Step 1) |
| P3-2 | Widget keyboard reordering + `aria-live` announcements | 3 | ✅ done (Step 3) |
| P3-3 | FilterSelect combobox/listbox semantics + arrow keys | 3 | ✅ done (Step 2) |
| P3-4 | Accessible names for search inputs (placeholder-as-label fixes) | 3 | ✅ done (Step 5) |
| P3-5 | Lightbox dialog focus management (focus on open, restore on close) | 3 | ✅ done (Step 1) |

### Implementation log

*(updated as work lands — newest entries first)*

**Steps 4 + 5 — Data completeness end-to-end + a11y labels · 2026-11-04** *(P1-B1, P1-A1, P1-B2, P1-B3 · completes P3-4)*

- **`src/routes.rs` (backend)** — `GET /db/:table` now accepts `?limit=` (default and hard ceiling `TABLE_ROWS_MAX_LIMIT = 10_000`, far beyond any realistic personal wardrobe) and returns the table's total row count in an `X-Total-Count` header; `GET /outfits` reports its total the same way. Truncation is now *detectable* by clients instead of silently dropping rows (P1-B1).
- **`frontend/src/api/client.ts`** — refactored around `fetchWithHeaders()` so list endpoints can read response headers without duplicating the 401-refresh logic; new `listClothingItemsWithTotal()` / `listOutfitsWithTotal()` plus a defensive `readTotalCount()` (missing header → `null` = "unknown", never "complete") (P1-B2).
- **`frontend/src/lib/wardrobe.ts`** — store now tracks `itemTotal` / `outfitTotal` from those responses; exports pure `formatTruncationNotice(itemsLoaded, itemsTotal, outfitsLoaded, outfitsTotal)` used by all views (P1-B2/B3).
- **Views (P1-A1 + P1-B3)** — the hard-coded "capped at 100 rows" banner in InsightsView is gone; Insights, Clothes and Outfits each render a data-driven notice (e.g. *"showing 500 of 612 clothing items — some data is not loaded…"*) that only appears when the server proves a snapshot is incomplete.
- **P3-4 completed** — `aria-label` added to the Clothes search input and both Settings inputs (Gemini API key, local Qwen URL). FilterBar's date inputs already had labels; clothing-form fields are properly wrapped in `<label>` elements (verified, no change needed).
- **Docs kept accurate with behavior changes:** README endpoint table updated (`?limit=` up to 10,000 + `X-Total-Count`); REPO_DESCRIPTION §4.5 SQL snippets and limit description updated.
- **Regression caught & fixed during integration:** the client-contract change broke three existing specs that mock `api` (wardrobe store spec, Insights smoke spec, period-filter spec) — all mocks migrated to `list*WithTotal`, plus a new test asserting the store stores server-reported totals (`itemTotal: 42 / outfitTotal: 7`).
- **Bonus fix (pre-existing):** `npm run typecheck` was failing before this work with 7 errors in `SettingsView.spec.ts` — its mock settings objects predate the Discord feature and omit required `discord_user_id`. Rewrote the spec around a complete `settings()` fixture; typecheck is now clean.
- **Verified:** `cargo check` + full `cargo test` green (31/31 backend tests); frontend 190/190 tests across 12 files; `tsc --noEmit` clean; production build clean.

**Step 3 — Widget keyboard reordering + live announcements · 2026-11-04** *(P3-2)*

- **`insights/WidgetCard.vue`** — each widget's `⋮` menu now offers **Move up / Move down**, giving mouse, keyboard and screen-reader users a parity path to drag-and-drop reordering; new `'move': [direction]` emit.
- **`insights/WidgetGrid.vue`** — forwards the event per widget id (`'move': [id, direction]`).
- **`InsightsView.vue`** — handles moves (with edge clamping) and announces every reorder through a polite `aria-live="polite"` region: *"Wear Trend moved from position 3 to position 1 of 8."* Clearing the message first guarantees repeated identical announcements are read again. Drag reorders announce identically, so both input modes get feedback.
- **Verified:** full Vitest suite green (insights smoke spec exercises widget rendering/menus) and clean build.

**Step 2 — FilterSelect combobox/listbox semantics · 2026-11-04** *(P3-3)*

- **`insights/FilterSelect.vue`** — trigger button now carries `aria-haspopup="listbox"` (single) / `"true"` (multi) and live `aria-expanded`. Single-select mode is a proper APG listbox popup: real `<button role="option">` rows with `aria-selected`, roving tabindex (one tab stop on the trigger), ArrowUp/Down/Home/End navigation, Enter/Space to select, Escape closes **and returns focus to the trigger**, and keyboard opening (ArrowDown from the trigger) moves focus into the list. Multi-select keeps native checkboxes (already fully keyboard-operable) inside a named `role="group"`. Outside-click close now restores focus when it was inside the panel.
- **Verified:** full Vitest suite green and clean build.

**Step 1 — Outfits core loop + calendar accessibility · 2026-11-04** *(P2-A1, A2, A3, A4, A5, B1 · P3-1, P3-5; part of P3-4)*

- **`frontend/src/components/OutfitsView.vue`**
  - Today is preselected by default (`selectedDate` initializes to today's date key); any explicit user selection still takes precedence. The landing screen on the app's default tab is now immediately actionable (P2-A1).
  - New primary **"Log today's outfit"** action in the calendar header replaces the old "Today" jump button; jumps the month, selects today, and focuses/reveals the day panel (P2-A2).
  - Selecting any day on the stacked (<900px) layout now scrolls the day panel into view — selection always has an observable consequence on mobile (P2-A3).
  - Item picker is recency-sorted: recently-worn garments first, never-worn items last with a case-insensitive name tiebreak; each worn item shows a subtle "last worn …" hint. Empty-picker state now cross-links to the Clothes tab (P2-A4, P2-B1).
  - Try-on generation is no longer an opaque wait: elapsed-seconds readout + indeterminate progress bar (with `prefers-reduced-motion` fallback) and a real **Cancel** button backed by an `AbortController` wired through `api.generateOutfitPreview(signal?)`. Abort is treated as user intent, not an error (P2-A5).
  - Calendar converted to a proper ARIA grid: day cells are focusable `<button role="gridcell">` elements with meaningful labels (e.g. "June 4, 2 outfits logged"), roving tabindex, and full keyboard navigation — arrow keys by day/week, Home/End within the week, PageUp/PageDown to switch months (P3-1).
  - Lightbox dialog focus lifecycle: focus moves into the dialog on open and is returned to the trigger element on close (P3-5).
  - First-run welcome card with cross-linked CTAs; only shown while the wardrobe is genuinely empty, dismissible permanently via localStorage `cw.onboarding.v1.dismissed` (P2-B1).
  - Picker search input gained an accessible name (`aria-label`) (part of P3-4).
- **New: `frontend/src/lib/picker.ts`** — pure helpers `buildLastWornMap`, `sortByRecency`, `formatLastWorn`; and **`picker.spec.ts`** with 12 unit tests.
- **Bug caught during testing:** the new date-key regex lacked capture groups (`` /\d{4}/ `` matches but exposes no groups), producing `NaN` dates; fixed to `/^(\d{4})-(\d{2})-(\d{2})$/` and strengthened the affected test to assert against an `"Invalid Date"` regression.
- **Verified:** full Vitest suite green (189/189 across 12 files, up from 177) and clean production build (`vite build`).

---

---

## 1. Silent data truncation with contradictory cap messaging

### 1.1 What is happening

The backend imposes a hard cap on every generic table read, and the frontend's awareness of that cap is wrong about both the *number* and the *threshold*. Three sources describe three different limits:

**Backend — `src/routes.rs` (the source of truth):**

```rust
// src/routes.rs:130-141  (GET /db/:table)
let query = format!(
    "SELECT to_jsonb(t) AS row FROM public.\"{}\" t WHERE t.user_id = $1 ORDER BY 1 LIMIT 500",
    quoted_table
);
```

The cap is **500 rows**, and the ordering is `ORDER BY 1` — i.e. by first column. For `clothing-items` the first column is the UUID primary key, so "the first 500 rows" is not chronological or in any user-meaningful order: it is an *arbitrary subset* of the wardrobe that happens to sort low by UUID string comparison.

**Frontend — `frontend/src/components/InsightsView.vue`:**

```ts
// InsightsView.vue:108  (fires the warning)
const itemsCapped = computed(() => (dataset.value?.items.length ?? 0) >= 100);
```

```html
<!-- InsightsView.vue:30-32 -->
<div v-if="itemsCapped" class="cap-notice">
  Showing the first 100 clothing items — the API is capped at 100 rows per table.
</div>
```

**README.md (endpoint documentation):**

> `GET /db/:table` — Read rows **(max 100)** / insert a row

So: the server says **500**, the UI says **100**, and the README says **100**. The banner also fires at the wrong moment — any user with 100–499 items sees "the API is capped" while actually receiving up to five times more data than the message implies.

### 1.2 Why this matters (abstract analysis)

Silent truncation is the worst class of failure mode in a data product, because it fails *invisibly*: nothing errors, nothing blocks, and the user has no way to know their view of reality is partial. The consequences cascade:

- **Every tab is affected, not just Insights.** `listClothingItems()`, `listOutfits()`, colors, categories, subcategories, and brands all flow through `/db/:table` (`frontend/src/api/client.ts`). A wardrobe with >500 items is silently incomplete in the Clothes grid, the Outfits day-panel picker, *and* Insights simultaneously.
- **Analytics are presented as truth over partial data.** The Insights tab computes cost-per-wear rankings, Pareto 80/20 callouts ("Top 20% of items account for X% of wears"), dusty-item warnings, and pairings from whatever snapshot the store holds — then displays them with full confidence. If a quarter of the wardrobe was never loaded, every one of those "insights" is wrong by construction, and the user will act on it (buy/sell/retire garments based on cost-per-wear).
- **The truncation is non-deterministic.** Because ordering is by UUID PK, *which* items are missing varies per table in ways no one can reason about. "Showing the first N" is not even an accurate description of what happens.
- **Trust asymmetry.** The app's own copy elsewhere works hard to build trust (destructive-action confirmations, undo toasts, error classification with suggestions). A hidden data cap undercuts all of it: once a user discovers their numbers were wrong, they will discount *all* future numbers the app shows.

### 1.3 Impact scope

| Surface | Symptom at scale (>cap items) |
|---|---|
| Clothes grid | Some garments silently absent from the list and search results |
| Outfits picker | Absent garments cannot be attached to outfits; no error, they just don't exist in the UI |
| Insights widgets | All metrics computed over an arbitrary partial subset; callouts may name "top" items that are merely early-UUID items |
| Cap banner (Insights) | Fires at 100 items with a wrong number and wrong framing ("first 100") |

### 1.4 Proposed fix — phased

**Phase A — Immediate honesty layer (frontend-only, < 1 day)**

1. Stop stating false numbers. Replace the hard-coded "capped at 100" copy with wording that is true under current behavior until Phase B lands, e.g.: *"This view works from a bounded snapshot of your wardrobe; some items may not be loaded."*
2. Gate confidence in Insights while data is partial: prefix or badge headline stats (SpendStats, StatCard, InsightsCallout) with "partial data" so no widget presents truncated results as complete.

**Phase B — Transparency via the API contract (1–2 days)**

3. Add completeness signals to list responses for `/db/:table` and `/outfits`: at minimum a total row count (`X-Total-Count` response header or an `?include_count=1` query param), and ideally cursor parameters so clients can page.
4. Extend the shared store (`frontend/src/lib/wardrobe.ts`) with `truncated: boolean` / `totalCount: number | null` per collection, set from those signals.
5. Surface one persistent, actionable notice in each data view when truncated — e.g. *"Showing 500 of 612 items"* — rather than only in Insights.

**Phase C — Correctness fix (backend + frontend pairing, ~3–5 days)**

6. Introduce cursor pagination (`?cursor=<id>&limit=...`) on `/db/:table` and `/outfits`; remove the hard cap for a user's own data or raise it far beyond any realistic personal wardrobe (the outfits list already has no LIMIT — parity argues for removing it).
7. Make ordering deterministic and meaningful: `created_at DESC, id` so "first N" means "newest N".
8. Have the store page in on demand, or at least load top-N with an explicit count, and verify with a seeded >500-item fixture that Clothes/Outfits/Insights all agree on completeness.

### 1.5 Success criteria

- The cap number appears in **exactly one source of truth** (the API contract) and every other mention is generated from it.
- A user can always answer "how much of my data is loaded, and is it complete?" from any tab.
- No Insight statistic is displayed without qualification when computed over a partial dataset.

---

## 2. The daily logging loop is not the app's most obvious feature

### 2.1 What is happening

The product's retention engine is a habit: log what you wore, every day. Every extra step in that loop is a tax paid up to 365 times a year — and the current implementation taxes it at four separate points.

**a) The app opens on the logging surface but does not select the date that matters.**

The default route is `#outfits` (`frontend/src/lib/router.ts`, fallback tab `'outfits'`). But no day is preselected:

```ts
// OutfitsView.vue:481
const selectedDate = ref<string | null>(null);
```

So the first-run and every-return experience is an empty side panel with the hint *"Select a day on the calendar to add an outfit."* `goToday()` exists as a button, but nothing calls it on load. "Log today" — the action that accounts for the large majority of uses — requires: click cell → (find form) → select items → submit. That is four interactions where one would do.

**b) On mobile, selecting a day produces no visible feedback.**

Below 900px the layout stacks (`.layout { flex-direction: column }` in `OutfitsView.vue` styles): calendar on top, the 360px day-panel below it — off-screen. Tapping a day cell mutates `selectedDate` and paints a border, but the panel that appeared is out of view; nothing scrolls it into place or presents it as a bottom sheet. On a phone, the tap *feels* dead even though state changed.

**c) The item picker is an unsorted wall of every garment.**

The picker (`filteredPickerItems` in `OutfitsView.vue`) lists all clothing items — up to 500 of them — with no ordering signal: no "worn recently", no favorites, no per-category memory. A user logging daily must re-find the same jeans and shirt through the same unranked list every day. This is the single most repeated micro-friction in the app.

**d) The AI try-on generation offers 20 seconds of opaque waiting.**

The generate button reads *"Generating outfit preview (10–20s)..."* while `isGeneratingPreview` disables it — but there is no progress indicator, elapsed-time feedback, or cancel path for up to twenty seconds. A long, invisible task with an unresponsive control reads as a hang; users will stop using the feature rather than wait blindly.

**e) First-run onboarding points nowhere.**

A new account lands on this *empty* Outfits tab. There is no guidance that the wardrobe must be populated first — the Clothes tab's empty state ("No clothing items yet. Add your first one with the form.") is only discoverable by manually navigating away. The natural first step (add clothes) is invisible from the landing surface, and the picker's "No clothing items yet" message hints at it without linking to it.

### 2.2 Why this matters (abstract analysis)

- **Habit mechanics.** Retention for a daily logger is determined by friction-on-the-happy-path, not by feature count. Each extra interaction has compounding odds of skipping; three skips in a row is roughly the point where the habit (and with it, all downstream value — Insights needs logged outfits to have anything to say) dies.
- **The default screen contradicts the mental model.** A calendar app that opens on "no day selected" asks the user to solve a state problem ("what am I looking at?") before they can do anything. The dominant case (today) should be the *default* state, not a button press away from it.
- **Mobile is where daily logging actually happens.** Outfit logging is the one flow with genuine phone ergonomics (you're dressing in the morning). A flow that feels broken on mobile is a flow users will stop doing — even if desktop works.
- **Recency is free intelligence the app already has.** The store holds `outfits` with their item joins; "last worn" is derivable without any schema change. Not using it means the picker treats day 30 exactly like day 1.

### 2.3 Proposed fix — phased

**Phase A — Friction removal (frontend-only, days)**

1. **Preselect today by default.** On first mount (and when returning with no explicit selection), set `selectedDate` to today's date key. Respect an explicitly chosen different day; only *default* to today when the user hasn't chosen anything. One line of state initialization removes the most common extra step and makes the landing screen immediately actionable.
2. **Add a "Log today's outfit" quick action** in the calendar header next to `Today` — explicit, one-tap, no ambiguity about which date is being logged.
3. **Make day selection visible on mobile:** when a cell is tapped below the layout breakpoint, scroll the day-panel into view (or present it as a bottom sheet). Selection must always have an observable consequence.
4. **Recency ordering in the picker.** Sort `filteredPickerItems` by last-worn descending (derived from the outfits join; fallback to creation order), with an optional "Worn recently" section pinned at top. No schema migration required.
5. **Generation progress as first-class state:** elapsed-time readout, a cancel affordance, and keep the draft visible while generating so the 10–20s wait is legible rather than mysterious.

**Phase B — Onboarding (frontend, days)**

6. **First-run checklist**, persisted in localStorage: "1. Add your clothes → 2. Log an outfit → 3. See insights." Each step links to its tab; completing a step checks it off. This converts the currently invisible dependency order (items before outfits before insights) into visible guidance.
7. **Cross-link empty states:** the Outfits picker's "No clothing items yet" should be a button/link to the Clothes tab, not just a sentence.

### 2.4 Success criteria

- A user returning on day N can log today's outfit in ≤ 3 interactions (open app → pick items → submit), with zero discovery steps.
- On mobile, every calendar interaction produces feedback within the viewport.
- The picker's top of list contains the garments actually most likely to be worn again.
- A brand-new account completes its first logged outfit without leaving the tab that told them what to do — or without being told where to go.

---

## 3. Custom interactive controls lack semantics (accessibility)

### 3.1 What is happening

The in-house Material Design 3 system means every interactive surface was hand-rolled with zero third-party UI dependencies — impressive engineering, but several of those surfaces have **no semantic identity at all**: no roles, no labels, no keyboard paths. The gaps cluster exactly where the product's value flows: picking days, picking items, arranging the dashboard.

**a) Calendar day cells are clickable divs.**

```html
<!-- OutfitsView.vue:27-37 -->
<div
  v-for="cell in cells"
  :key="cell.key"
  class="day-cell"
  @click="selectDay(cell.key)"
>
```

No `role`, no `tabindex`, no keyboard handler, and no accessible name describing the day's contents. A screen-reader user hears a wall of unlabeled boxes; a keyboard user cannot focus one at all. Since selecting a day is the *only* way to reach the outfit form, **keyboard and assistive-technology users cannot log an outfit in this app.**

**b) Widget reordering is mouse-only drag-and-drop.**

```html
<!-- insights/WidgetCard.vue:13-22 -->
<button class="widget-card__handle" type="button" title="Drag to move"
  aria-label="Drag to move"
  @mousedown="dragEnabled = true"
  @mouseup="dragEnabled = false">
```

Reordering is native HTML5 DnD gated behind a `mousedown`-enabled handle. There is no keyboard alternative, and when reorders *do* happen (by mouse), nothing announces the new arrangement — no live region, no focus change. The widget's own `⋮` menu (Configure / Remove) shows the pattern for adding controls already exists; it simply lacks move affordances.

**c) Filter dropdowns are unmanaged custom popovers.**

`insights/FilterSelect.vue` renders a `<button>` that toggles an absolutely-positioned options div: no `aria-haspopup`, no `aria-expanded`, no `role="listbox"`/`option`, no arrow-key navigation, no roving focus, and close-on-outside-`mousedown` is the only dismissal path. The multi-select checkboxes inside are technically tab-reachable, but the container doesn't manage them as a widget — so keyboard operation of "filter by color" is guesswork and screen readers get an unlabeled stack of options.

**d) Placeholder text used as the field label.**

```html
<!-- ClothesView.vue:182-187 -->
<input v-model="clothesSearch" type="search"
  class="clothes-search-input"
  placeholder="Search clothes by name, brand, or notes…" />
```

The same pattern appears in the Outfits picker search. Placeholder-as-label fails at fill (the label vanishes), provides no persistent accessible name on some platforms, and violates basic form labeling guidance. (Credit where due: the *brand* search input does carry an `aria-label` — the codebase knows how; it just isn't applied consistently.)

**e) Dialog focus management is incomplete.** The lightbox modal in `OutfitsView.vue` does well here (`role="dialog"`, `aria-modal`, Escape handling, click-outside close), but it neither moves focus into itself on open nor returns focus to the trigger on close — two standard dialog behaviors that screen-reader and keyboard users rely on.

### 3.2 Why this matters (abstract analysis)

- **These are not edge cases.** "Keyboard-only" includes power users with trackpads/mice fatigue, switch devices, and voice-control setups; screen readers cover a large population of the app's likely demographic (personal-finance/wardrobe trackers skew to people who invest in their data). When an entire primary flow — *logging an outfit* — is unreachable without a mouse, that is not a polish issue; it is an exclusion.
- **The gaps are systemic, not incidental.** Every instance follows the same root cause: *interactive divs instead of semantic elements*, inherited from building a zero-dependency design system from scratch with no accessibility contract in the component checklist. Fixing only the instances will regress next quarter when the next custom component ships.
- **The cost is now, not later.** The fixes are small (real buttons, ARIA attributes, two extra menu items) and ride on components that already exist. Deferring them compounds: every future screen built on these patterns inherits the same invisibility.

### 3.3 Proposed fix — per control + system-level prevention

**Per-control fixes:**

1. **Calendar → real ARIA grid.** Each cell becomes a focusable `<button>` (or `role="gridcell"` with `aria-selected`) carrying an accessible name that states the full meaning, e.g. *"June 4, 2 outfits"* / *"June 9"*. Add arrow-key day navigation, Home/End for first/last of month, and PageUp/PageDown or explicit controls for months. This single fix unblocks the entire logging flow for non-mouse users.
2. **Widget reordering → keyboard parity.** Add "Move up" / "Move down" items to each widget's existing `⋮` menu (two small buttons; reorder logic already exists in `InsightsView.vue`). Announce position changes via an `aria-live="polite"` region ("Wear Trend moved to position 3 of 8"). Keep native DnD as the mouse fast path.
3. **FilterSelect → proper combobox/listbox pattern.** Trigger button: `aria-haspopup="listbox"`, `aria-expanded`. Panel: `role="listbox"` (multi) or `menu`/radio semantics (single), options with roving tabindex, ArrowUp/Down + Home/End navigation, Escape closes **and returns focus to the trigger**, Enter/Space selects. Where a single-select case is pure preference state, consider native `<select>` — free semantics, zero maintenance.
4. **Labels → real labels.** Visible `<label>`s (or at minimum `aria-label`) for every search input; keep placeholders as secondary hint text only. Apply uniformly across ClothesView, OutfitsView picker, and Settings inputs.
5. **Dialogs → complete focus lifecycle.** Move focus into the lightbox on open; return it to the triggering element on close. (The other dialogs — delete confirmations, widget palette — get the same treatment.)

**System-level prevention:**

6. Add a short **"semantic controls" checklist** (button-not-div, role + label, keyboard path, focus management, live announcements) to the design-system documentation in `frontend/src/ui/` so every future M3 component is gated on it before merge.
7. Extend the existing Vitest suite with interaction tests: "select a day via keyboard", "reorder a widget via keyboard", "open filter and select option via arrow keys". The repo already has 165 well-organized tests — none cover keyboard/a11y paths yet, which is precisely where these regressions will hide.

### 3.4 Success criteria

- A user can complete the full core loop (add item → log outfit on a chosen day → reorder dashboard) **without touching a mouse**.
- Screen-reader users receive meaningful names for every interactive surface ("June 4, 2 outfits", "Filter colors: blue selected").
- No custom control exists without a documented keyboard path and focus behavior.

---

## 4. Secondary concerns (watchlist — did not make top 3)

| Concern | Observation | Suggested action |
|---|---|---|
| **Sync opacity** | Background refreshes can fail *silently* after initial load (`wardrobe.ts` error policy deliberately keeps the cached snapshot, with no user-visible signal), and Discord import runs automatically on every mount of the Outfits view (`OutfitsView.vue:696`, `syncDiscord(false)` in `onMounted`) with no "last synced" timestamp anywhere. | Show a last-synced indicator in the Outfits header; surface background-refresh failures as a dismissible, non-blocking notice ("Data may be out of date — retry"). |
| **Information architecture** | "Tables" is a raw PostgreSQL schema explorer exposed as a first-class nav tab next to end-user features, and "Settings" contains only AI try-on + Discord configuration (no account or theme settings exist). End users meet database terminology in primary navigation. | Demote Tables behind an "Advanced/Developer" affordance (or fold it into Settings); rename the Settings scope so its contents are discoverable ("AI & integrations"). |
| **Form validation timing** | `ClothingItemForm` validates on submit with a single generic error string rendered at form bottom; mid-form inline "+ Add new…" subflows (color/category/subcategory/brand) each make their own API call and surface failures in the same undifferentiated slot. | Field-level inline errors with `aria-describedby`; scope micro-flow failures to their own field rather than the global footer. |

---

## 5. Suggested sequencing summary

| Phase | Workstream | Items | Est. effort |
|---|---|---|---|
| **Week 1 — quick wins** | Problem 2 | Preselect today; "Log today's outfit" quick action; fix cap-banner copy (P1-A); visible search labels | Days |
| | Problem 3 | Calendar buttons + keyboard nav; placeholder→label fixes; dialog focus lifecycle | Days |
| **Weeks 2–3 — trust & habit** | Problem 1 | API completeness signals (`total_count`/cursor params); store `truncated` state; persistent per-view truncation notice | 1–2 days backend + frontend pairing |
| | Problem 2 | Mobile day-selection feedback (scroll/bottom sheet); picker recency ordering; generation progress + cancel | Days |
| | Problem 3 | Widget keyboard reorder + live announcements; FilterSelect listbox pattern; a11y interaction tests | Days |
| **Weeks 3–4 — structural** | Problem 1 | Pagination on `/db/:table` & `/outfits`; deterministic `created_at DESC` ordering; >500-item seeded fixtures in test suite | 3–5 days paired work |
| | Onboarding (P2) | First-run checklist + cross-linked empty states | Days |

**Ordering rationale:** Week 1 items are near-zero-risk and remove the most frequent daily frictions plus the false-number trust wound. Phase B/C of Problem 1 is sequenced after transparency exists so that any interim state is at least *honest*. The a11y fixes ride on components that already exist, so they cost little now and grow expensive with every future screen built on the same patterns.

---

## Appendix — referenced files

| File | Relevance |
|---|---|
| `src/routes.rs` (L129–147) | 500-row cap + `ORDER BY 1` on generic table reads |
| `frontend/src/components/InsightsView.vue` (L30–32, L108) | "capped at 100" banner; threshold computed |
| `README.md` (endpoint table) | Documents the cap as "max 100" — third number in circulation |
| `frontend/src/lib/wardrobe.ts` | Shared store: no completeness/truncation state; silent background-refresh error policy |
| `frontend/src/api/client.ts` | All list endpoints funnel through `/db/:table`; image proxy token handling |
| `frontend/src/components/OutfitsView.vue` (L27–37, L481, L693–697) | Div-based calendar cells; null initial selection; auto Discord sync on mount |
| `frontend/src/lib/router.ts` | Default tab = `outfits`; deep-link param handling |
| `frontend/src/components/ClothesView.vue` (L182–187, L1094+) | Placeholder-only search label; responsive stack layout |
| `frontend/src/components/insights/WidgetCard.vue` (L13–22) | Mouse-gated DnD handle |
| `frontend/src/components/insights/FilterSelect.vue` | Unmanaged custom dropdown, no listbox semantics |
| `frontend/src/lib/errors.ts` | Error classification with contextual suggestions (existing strength to build on) |
