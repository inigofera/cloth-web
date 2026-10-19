# Insights Tab — Planning Document

Status: **planning / exploration** (no code written yet)
Date: 2026-09-17

---

## 1. Goal

Add a new **Insights** tab to Cloth Web that shows modular, user-configurable widgets with metrics and analytics about the clothes and outfits the user has stored. Users can add, remove, reorder, and filter widgets; each widget derives its numbers from the existing database tables.

---

## 2. Current state (what we build on)

### Stack
- **Frontend:** Vue 3 + TypeScript + Vite (`frontend/`), custom Material-style UI kit in `frontend/src/ui/` (`MdAppBar`, `MdNavDrawer`, `MdButton`, icons, design tokens). No charting library installed.
- **Backend:** Rust/Axum + sqlx against Supabase Postgres (`src/routes.rs`). Auth via Supabase JWT through the existing `AuthUser` middleware; every user-scoped query filters on `user_id`.
- **Tabs today** (in `App.vue`): Clothes, Outfits, Tables — a nav drawer with an `activeTab` ref and one view component per tab.

### Data model (all tables in schema `public`)

| Table | Scope | Columns |
|---|---|---|
| `clothing-items` | per user (`user_id`) | `id` (uuid), `name`, `category_id` (int → categories), `subcategory_id` (int → subcategories), `color_id` (string → colors), `brand_id` (uuid → brands), `purchase_price` (numeric, nullable), `owned_since` (date/timestamptz, nullable), `origin` (text, nullable), `laundry_impact` (text, nullable), `repairable` (bool, nullable), `notes` (text, nullable), `image_path` (text → Supabase Storage key, nullable), `is_active` (bool), `wear_count` (int, nullable/0), `created_at`, `updated_at` |
| `outfits` | per user (`user_id`) | `id` (uuid), `date` (timestamptz — the day the outfit was worn/logged), `notes` (nullable), `image_path` (nullable), `is_active` (bool), `created_at`, `updated_at` |
| `outfit-items` | join table | `outfit_id` (uuid) ↔ `clothing_item_id` (uuid) |
| `colors` | shared reference | `id` (string, lowercase name), `hex_value` (`#RRGGBB`, nullable) |
| `clothing-categories` | shared reference | `id` (serial int), `name` |
| `clothing-subcategories` | shared reference | `id` (serial int), `name`, `category_id` |
| `brands` | shared reference | `id` (uuid), `name` |

### Key API facts that constrain the design
1. **Generic table endpoint is capped at 100 rows** — `GET /api/db/:table` (`get_table_rows`) does `LIMIT 100`. Client-side aggregation over a whole wardrobe would silently truncate past 100 items. **A dedicated uncapped (or high-limit) insights endpoint for `clothing-items` is required.**
2. **Outfits are already fully available** — `GET /api/outfits` (`list_outfits`) has no row cap and embeds each outfit's full item list via the `outfit-items` join. So versatility/co-occurrence metrics can be derived client-side from this one payload; no new endpoint needed for outfits or the join table.
3. **Reference tables are tiny** — colors/categories/subcategories/brands already fetched via `/db/colors`, etc.
4. **No charting library exists yet** — widgets must either hand-roll SVG/CSS charts (fits the existing MD design tokens, zero deps) or add a library later.

---

## 3. Implementation approach

### 3.1 Tab wiring (trivial)
In `App.vue`:
- Add `{ id: 'insights', label: 'Insights', icon: 'chart' }` to `navItems`.
- Add `<InsightsView v-else-if="activeTab === 'insights'" />`.

### 3.2 Frontend structure

```
frontend/src/
├── components/insights/
│   ├── InsightsView.vue        # tab root: global filter bar + widget grid + "add widget" palette
│   ├── InsightCard.vue         # generic wrapper: title, description, loading/error/empty states, size class
│   └── widgets/
│       ├── registry.ts         # WidgetDefinition[] — single source of truth for available widgets
│       ├── WardrobeOverview.vue
│       ├── CategoryBreakdown.vue
│       ├── ColorPalette.vue
│       └── ... (one component per widget)
├── lib/insights/
│   ├── data.ts                 # fetches the full InsightData bundle once, caches it
│   ├── metrics.ts              # pure aggregation functions (unit-testable, no Vue imports)
│   └── layout.ts               # localStorage persistence of the user's widget layout
```

### 3.3 Widget contract

Every widget is a self-describing module so the palette, grid, and filters all work generically:

```ts
interface WidgetDefinition {
  id: string;                  // 'cost-per-wear-top10'
  title: string;               // "Cost per wear — top 10"
  description: string;         // shown in the palette + tooltip
  category: InsightCategory;   // 'wardrobe' | 'outfits' | 'value' | 'sustainability' | 'data-quality'
  size: 'sm' | 'md' | 'lg';    // grid span (CSS grid areas)
  filters?: FilterKey[];       // which global filters this widget reacts to
  compute(ctx: InsightData, f: GlobalFilters): WidgetResult;
}

interface InsightData {         // fetched once per tab mount
  items: ClothingItem[];        // ALL of the user's items (uncapped)
  outfits: Outfit[];            // with embedded items[] from /api/outfits
  colors: Color[]; categories: ClothingCategory[];
  subcategories: ClothingSubcategory[]; brands: Brand[];
}

interface GlobalFilters {
  dateFrom?: string; dateTo?: string;   // applies to outfits.date (and item dates where sensible)
  category_id?: number | null;
  color_id?: string | null;
  brand_id?: string | null;
  activeOnly?: boolean;
}
```

- `metrics.ts` holds pure functions (`topN`, `histogram`, `cooccurrencePairs`, `streaks`, …) that widgets call. Keeping math out of components makes it testable and reusable across widgets.
- Each widget component is dumb: it calls its definition's `compute()` inside a `computed()`, renders the result with charts, and shows empty/error states via `InsightCard`.

### 3.4 Data layer & required backend change

**Frontend:** on tab mount, fetch in parallel:
1. `GET /api/insights/items` — **new endpoint** (see below), all items for the user, no cap.
2. `GET /api/outfits` — existing, uncapped, includes embedded items.
3. `/db/colors`, `/db/clothing-categories`, `/db/clothing-subcategories`, `/db/brands` — existing reference fetches (already done in ClothesView; reuse).

**Backend (Rust):** add to `src/routes.rs` + register in `main.rs`:

```rust
// GET /api/insights/items  → all clothing-items rows for the authenticated user, no LIMIT
pub async fn list_insight_items(State(state): State<Arc<AppState>>, AuthUser(user_id): AuthUser) -> Json<Vec<JsonValue>> { ... }
```

Same pattern as `list_outfits` (`SELECT to_jsonb(ci) FROM "clothing-items" ci WHERE ci.user_id = $1`). No schema changes, no new auth — the existing `AuthUser` middleware guarantees user scoping.

**Why not just raise the generic 100-row cap?** The `/db/:table` endpoint also powers TablesView; changing its global behavior is riskier than adding one small dedicated route. (Alternative: add a `?limit=` query param to the generic endpoint — acceptable, but the dedicated route is cleaner and self-documenting.)

**Scaling note:** client-side aggregation over a personal wardrobe (hundreds–low thousands of items) is trivial in JS. If data ever grows large, move heavy widgets to server-side SQL aggregates (`GROUP BY`, `percentile_cont`) behind `/api/insights/*` endpoints — the widget contract doesn't change, only where `compute()` runs.

### 3.5 Charts

No chart library is installed today. Options:
- **Recommended for v1:** hand-roll lightweight SVG/CSS charts (bars, donuts, heatmaps, sparklines, scatter) using the existing design tokens in `ui/styles/tokens.css`. Zero dependencies, visually consistent with the MD kit, and these widgets need only simple chart types.
- **Later if needed:** add Chart.js + vue-chartjs or ECharts for interactive line/scatter charts (tooltips, zoom).

### 3.6 Layout & persistence

- User's layout stored in `localStorage` under a versioned key:
  ```ts
  // localStorage['insights.layout.v1'] = { widgets: [{ id: 'wardrobe-overview', size: 'lg' }, ...] }
  ```
- **Add-widget palette:** modal listing all registered widgets with search + category filter; each entry shows title, description, and a preview icon.
- **Reorder:** up/down buttons per widget in v1 (drag-and-drop is a nice-to-have later).
- **"Reset to defaults"** button restores the curated default layout.
- Optional future: an `insight-layouts` DB table for cross-device sync (localStorage is per-browser only).

### 3.7 UX details
- Loading skeletons while the data bundle loads; per-widget error isolation (one failing widget must not break the grid).
- Empty states with a CTA ("No outfits logged yet — add one in the Outfits tab").
- Global filter bar at top of the tab: date range, category, color, brand, active-only. Widgets declare which filters they support; unsupported filters are visually disabled for that widget (or simply don't affect it).

---

## 4. Widget catalog (thorough list)

Derived from every field in the tables above. Filters legend: **D** = date range, **C** = category/subcategory, **K** = color, **B** = brand, **A** = active-only.

### Group A — Wardrobe overview & composition

| # | Widget | What it shows | Data source (table.field) | Chart/UI | Filters |
|---|--------|---------------|---------------------------|----------|---------|
| A1 | **Wardrobe at a glance** | Stat tiles: total items, active items, outfits logged, total wears, wardrobe value (Σ purchase_price), avg cost per wear | `clothing-items` (count, is_active, purchase_price, wear_count), `outfits` (count) | Tile row | A |
| A2 | **Category breakdown** | Item count & share per category; drill-down to subcategories on click/hover | `clothing-items.category_id/subcategory_id`, reference tables | Donut + bar list | C, K, B, A |
| A3 | **Color palette** | Swatch grid of all colors with counts and % share; dominant color callout; optional warm/cool split computed from `hex_value` hue | `clothing-items.color_id`, `colors.hex_value` | Swatch mosaic + bars | C, B, A |
| A4 | **Brand leaderboard** | Horizontal bars: items per brand, plus total spend and avg price per brand | `clothing-items.brand_id/purchase_price`, `brands.name` | Ranked bars | K, A |
| A5 | **Wardrobe growth timeline** | Cumulative item count over time; "added last 30 days" callout | `clothing-items.created_at` | Area/line chart | D |
| A6 | **Item age distribution** | Histogram of item ages (owned <1y, 1–2y, 2–5y, 5+); oldest & newest items callouts; % missing `owned_since` | `clothing-items.owned_since` | Histogram + stat chips | C, K, B, A |
| A7 | **Origin breakdown** | Items by country/region of origin (free-text grouping) | `clothing-items.origin` | Bar list (map later) | C, K, B, A |
| A8 | **Wardrobe wall** | Mosaic of item thumbnails colored by dominant color; click-through to Clothes tab; flags items without images | `clothing-items.image_path/color_id/is_active`, `colors.hex_value` | Thumbnail grid | C, K, B, A |

### Group B — Wear & usage

| # | Widget | What it shows | Data source | Chart/UI | Filters |
|---|--------|---------------|-------------|----------|---------|
| B1 | **Most-worn items (top N)** | Ranked list with wear_count and cost per wear; thumbnails | `clothing-items.wear_count/purchase_price/image_path` | Ranked list | C, K, B, A |
| B2 | **Dormant / never-worn items** | Items with `wear_count = 0` (or very low) — "forgotten" inventory, sorted by age | `clothing-items.wear_count/owned_since/is_active` | List + count stat | C, K, B |
| B3 | **Wear-count distribution** | Histogram of wear counts across wardrobe; mean/median callouts | `clothing-items.wear_count` | Histogram | C, K, B, A |
| B4 | **Cost-per-wear leaderboard** | Cheapest and most expensive per-wear items (`purchase_price / max(wear_count,1)`); value-for-money insight | `clothing-items.purchase_price/wear_count` | Ranked list + stat chips | C, K, B, A |
| B5 | **Wear rate by category** | Average wear count per category — which categories actually get used | `clothing-items.wear_count/category_id` | Bar chart | K, B, A |
| B6 | **Versatility score / most-reused items** | How many outfits each item appears in; top-N "workhorse" items | `outfit-items` join (derivable from `/api/outfits` embedded items) | Ranked list | C, K, B |
| B7 | **Orphan items** | Active items that appear in zero outfits — candidates to style or remove | `outfit-items` + `clothing-items.is_active` | List + count | C, K, B |

### Group C — Outfits & calendar (all from `outfits.date`)

| # | Widget | What it shows | Data source | Chart/UI | Filters |
|---|--------|---------------|-------------|----------|---------|
| C1 | **Outfit activity heatmap** | GitHub-style 365-day calendar: outfits per day, intensity-colored | `outfits.date` | Calendar heatmap | D |
| C2 | **Outfits over time** | Line/bar of outfits per month (or week); most active months & seasons callout | `outfits.date` | Line chart | D |
| C3 | **Day-of-week distribution** | Which weekday you log/wear most; "weekend vs weekday" split | `outfits.date` → weekday | Bar/donut | D |
| C4 | **Streaks & rhythm** | Current streak, longest streak, avg outfits per week/month, gap since last outfit | `outfits.date` | Stat tiles + sparkline | — |
| C5 | **Outfit size distribution** | Pieces-per-outfit histogram; typical silhouette (avg items/outfit) | `outfit-items` via embedded items | Histogram + stat | D |
| C6 | **Outfit value over time** | Total purchase_price of each outfit's items; most expensive look; trend line | `outfits.date` + item `purchase_price` via join | Scatter/line + callout | D, K, B |
| C7 | **Color co-occurrence ("signature combos")** | Which color pairs appear together most across outfits (e.g., "blue+white ×12"); top-N pair list or small network graph | `outfit-items` join + `clothing-items.color_id`, `colors.hex_value` | Pair bars / network | D, C, B |
| C8 | **Outfit composition gaps** | % of outfits containing each category (top/bottom/shoes/outerwear); "X% of outfits have no footwear" callouts | `outfit-items` join + `clothing-items.category_id` | Stacked bar / checklist | D, K, B |
| C9 | **Brand mixing** | Brands that appear together in outfits; loyalty vs. mix indicator | `outfit-items` join + `clothing-items.brand_id` | Co-occurrence list | D, C, K |
| C10 | **Seasonal wardrobe** (derived) | Items appearing in outfits logged Oct–Mar vs Apr–Sep → seasonal vs all-season items; per-season item counts | `outfits.date` + join table | Season bars / split view | D, C, K, B |
| C11 | **Outfit filmstrip** | Chronological strip of outfit photos — visual history of your looks | `outfits.image_path/date` | Horizontal scroll strip | D |

### Group D — Value & money (all from `purchase_price`)

| # | Widget | What it shows | Data source | Chart/UI | Filters |
|---|--------|---------------|-------------|----------|---------|
| D1 | **Wardrobe value summary** | Total spend, avg/median price, most expensive single item, spend by category | `clothing-items.purchase_price/category_id` | Stat tiles + bars | C, K, B, A |
| D2 | **Price histogram** | Distribution of purchase prices (log-scale option); % missing price | `clothing-items.purchase_price` | Histogram | C, K, B, A |
| D3 | **Spend over time** | Cumulative or monthly spend by acquisition date — "what you've invested this year" | `clothing-items.owned_since/purchase_price` | Area chart | D, C, K, B |
| D4 | **Value-for-money quadrant** | Scatter: x = wear_count, y = purchase_price → quadrants: workhorses (cheap & worn), neglected luxury (expensive & rarely worn), fair deals, overpriced | `clothing-items.wear_count/purchase_price` | Scatter plot | C, K, B, A |

### Group E — Sustainability & care

| # | Widget | What it shows | Data source | Chart/UI | Filters |
|---|--------|---------------|-------------|----------|---------|
| E1 | **Laundry impact distribution** | Share of items per `laundry_impact` level; eco-score summary | `clothing-items.laundry_impact` | Donut + bars | C, K, B, A |
| E2 | **Repairability stats & repair candidates** | % repairable; "repair candidates" list = high wear_count + not repairable (replace soon) and worn-but-repairable (worth fixing) | `clothing-items.repairable/wear_count` | Stat + prioritized list | C, K, B, A |
| E3 | **Durability by brand/category** | Avg cost per wear grouped by brand or category — which brands hold up over time | `clothing-items.purchase_price/wear_count/brand_id/category_id` | Ranked bars | A |
| E4 | **Lifecycle / cull suggestions** | Items older than N years with low wears and inactive → retire/donate candidates | `clothing-items.owned_since/wear_count/is_active` | List + count | C, K, B |

### Group F — Data quality & gaps

| # | Widget | What it shows | Data source | Chart/UI | Filters |
|---|--------|---------------|-------------|----------|---------|
| F1 | **Completeness checklist** | % of items missing image / price / brand / category / color; progress bar + list of incomplete items with quick-fix links to Clothes tab | `clothing-items` nullability per field | Progress bar + list | — |
| F2 | **Wardrobe gap analysis** | Categories/subcategories under-represented relative to outfit needs (e.g., many outfits, few outerwear pieces); color gaps (outfits dominated by one color) | items vs `outfit-items` join comparison | Gap bars / callouts | — |
| F3 | **Data freshness** | Last item added, last outfit logged, edit activity from `updated_at`; "you haven't logged an outfit in X days" nudge | `clothing-items.created_at/updated_at`, `outfits.date` | Stat tiles + nudge banner | — |

### Group G — Composite & fun (built from other metrics)

| # | Widget | What it shows | Data source | Chart/UI | Filters |
|---|--------|---------------|-------------|----------|---------|
| G1 | **Wardrobe health score** | Single 0–100 gauge blending data completeness, active %, repairable %, median cost-per-wear; breakdown bars per component | composite of A/B/E/F metrics | Gauge + breakdown | — |
| G2 | **Style DNA card** | "Your signature": top category + top color + top brand + typical outfit size + dominant season → one shareable profile card | composite of A/C/D metrics | Profile card | — |
| G3 | **Notes keyword cloud** (speculative, low priority) | Frequent words in item/outfit notes (occasions, contexts) if users write meaningful notes | `clothing-items.notes`, `outfits.notes` | Word cloud / top-N list | D |

### Notes on derived metrics
- **Item age** = now − `owned_since`; **cost per wear** = `purchase_price / max(wear_count, 1)`; **season** = month of `outfits.date` (Northern-Hemisphere mapping — fine for the Amsterdam user, but should be configurable later).
- All null fields (`purchase_price`, `owned_since`, `brand_id`, …) must be handled gracefully: widgets show coverage percentages ("82% of items have a price") instead of failing.
- **Limitation:** `wear_count` is a counter with no timestamps, so "wears over time" trends are impossible without a schema change. Optional future table `item-wears(item_id, date)` (or logging wears when an outfit is created) would unlock true wear-over-time analytics — worth flagging as a follow-up, not needed for v1.

---

## 5. Phased rollout

**Phase 0 — Foundation (no visible widgets yet)**
- Backend: `GET /api/insights/items` endpoint + route registration.
- Frontend: tab wiring in `App.vue`, `InsightData` fetch layer, widget registry + contract, `InsightCard` wrapper, layout persistence (localStorage), add-widget palette, global filter bar.

**Phase 1 — MVP widgets (~8)**
A1 Wardrobe at a glance · A2 Category breakdown · A3 Color palette · B1 Most-worn items · B4 Cost-per-wear leaderboard · C1 Outfit activity heatmap · F1 Completeness checklist · B2 Dormant items.

**Phase 2 — Breadth**
A5 Growth timeline, A6 Item age histogram, D1–D4 value widgets, C2–C9 calendar/seasonal/co-occurrence widgets, E1–E4 sustainability widgets, F2–F3 quality widgets, B3/B5/B6/B7 usage widgets.

**Phase 3 — Polish & scale**
G1 Health score, G2 Style DNA card, C11 filmstrip, A8 wardrobe wall, network-graph co-occurrence; move heavy aggregates to SQL if data grows; optional `insight-layouts` table for cross-device sync; drag-and-drop reordering.

---

## 6. Risks & open questions
1. **100-row cap** on `/api/db/:table` — must be solved before any full-wardrobe aggregation (Phase 0 endpoint).
2. **Timezone handling** for `outfits.date` (timestamptz → local date key, same approach as OutfitsView's `outfitKey`).
3. **Null-heavy fields** — every widget needs missing-data handling and coverage stats.
4. **No wear timestamps** — limits time-based wear analytics; consider an optional `item-wears` log table later.
5. **localStorage-only layout** is per-browser; acceptable for v1, DB sync later.
6. **Charting approach** — hand-rolled SVG/CSS first (zero deps); revisit with a library only if interactive charts become necessary.
7. **Testing** — `metrics.ts` pure functions should get unit tests (would require adding vitest to devDependencies).
