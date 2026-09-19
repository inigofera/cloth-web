import type { ConfigField, WidgetDef } from './types';
import { RANGE_OPTIONS } from './types';
import StatCard from '../../components/insights/widgets/StatCard.vue';
import WearTrend from '../../components/insights/widgets/WearTrend.vue';
import TopWorn from '../../components/insights/widgets/TopWorn.vue';
import DustyItems from '../../components/insights/widgets/DustyItems.vue';
import DayOfWeek from '../../components/insights/widgets/DayOfWeek.vue';
import SeasonPattern from '../../components/insights/widgets/SeasonPattern.vue';
import CategoryDonut from '../../components/insights/widgets/CategoryDonut.vue';
import ColorBar from '../../components/insights/widgets/ColorBar.vue';
import BrandBar from '../../components/insights/widgets/BrandBar.vue';
import PriceHistogram from '../../components/insights/widgets/PriceHistogram.vue';
import WardrobeAge from '../../components/insights/widgets/WardrobeAge.vue';
import SpendStats from '../../components/insights/widgets/SpendStats.vue';
import CostPerWear from '../../components/insights/widgets/CostPerWear.vue';
import ExpensiveDusty from '../../components/insights/widgets/ExpensiveDusty.vue';
import LaundryMix from '../../components/insights/widgets/LaundryMix.vue';
import ImpactHotspots from '../../components/insights/widgets/ImpactHotspots.vue';
import ItemPairings from '../../components/insights/widgets/ItemPairings.vue';
import RepeatWears from '../../components/insights/widgets/RepeatWears.vue';
import InsightsCallout from '../../components/insights/widgets/InsightsCallout.vue';

/** Per-widget date range field (narrows the global filter). Factory so each def owns its own copy. */
const rangeField = (): ConfigField => ({ key: 'range', label: 'Period', kind: 'select', options: [...RANGE_OPTIONS] });

const defs: WidgetDef[] = [
  // ---- Usage habits ----
  {
    type: 'stat-card',
    title: 'Key number',
    category: 'usage',
    description: 'A single headline number: items, outfits, spend, streak and more.',
    defaultSize: 'sm',
    defaultConfig: { metric: 'total_items', range: 'all', dustyDays: 30 },
    configSchema: [
      {
        key: 'metric',
        label: 'Metric',
        kind: 'select',
        options: [
          { value: 'total_items', label: 'Total items' },
          { value: 'active_items', label: 'Active items' },
          { value: 'total_outfits', label: 'Outfits logged' },
          { value: 'total_wears', label: 'Total wears' },
          { value: 'never_worn', label: 'Never worn' },
          { value: 'dusty', label: 'Gathering dust' },
          { value: 'total_spend', label: 'Total spend' },
          { value: 'avg_price', label: 'Average price' },
          { value: 'cost_per_wear', label: 'Cost per wear' },
          { value: 'items_per_outfit', label: 'Items per outfit' },
          { value: 'current_streak', label: 'Logging streak' },
        ],
      },
      {
        key: 'dustyDays',
        label: 'Dusty after (days)',
        kind: 'number',
        min: 7,
        max: 365,
        showWhen: { key: 'metric', equals: 'dusty' },
      },
      rangeField(),
    ],
    component: StatCard,
  },
  {
    type: 'wear-trend',
    title: 'Wear activity',
    category: 'usage',
    description: 'How often you log outfits over time.',
    defaultSize: 'lg',
    defaultConfig: { granularity: 'week', metric: 'outfits', range: 'all' },
    configSchema: [
      {
        key: 'granularity',
        label: 'Granularity',
        kind: 'select',
        options: [
          { value: 'week', label: 'Weekly' },
          { value: 'month', label: 'Monthly' },
        ],
      },
      {
        key: 'metric',
        label: 'Count',
        kind: 'select',
        options: [
          { value: 'outfits', label: 'Outfits' },
          { value: 'items', label: 'Unique items' },
          { value: 'items_per_outfit', label: 'Items per outfit' },
        ],
      },
      rangeField(),
    ],
    component: WearTrend,
  },
  {
    type: 'top-worn',
    title: 'Most worn',
    category: 'usage',
    description: 'Your workhorse items, ranked by wears.',
    defaultSize: 'md',
    defaultConfig: {
      topN: 5,
      source: 'outfits',
      metric: 'total',
      minWears: 0,
      range: 'all',
      categoryId: '',
      subcategoryId: '',
    },
    configSchema: [
      {
        key: 'metric',
        label: 'Rank by',
        kind: 'select',
        options: [
          { value: 'total', label: 'Total wears' },
          { value: 'per_month', label: 'Wears per month' },
        ],
      },
      {
        key: 'source',
        label: 'Source',
        kind: 'select',
        options: [
          { value: 'outfits', label: 'Logged outfits' },
          { value: 'wear_count', label: 'Wear count field' },
        ],
      },
      { key: 'categoryId', label: 'Category', kind: 'category-select' },
      { key: 'subcategoryId', label: 'Subcategory', kind: 'subcategory-select', dependsOn: 'categoryId' },
      { key: 'minWears', label: 'Min wears', kind: 'number', min: 0, max: 50 },
      { key: 'topN', label: 'Show top', kind: 'number', min: 3, max: 15 },
      rangeField(),
    ],
    component: TopWorn,
  },
  {
    type: 'dusty-items',
    title: 'Gathering dust',
    category: 'usage',
    description: 'Items you have not worn recently — or ever.',
    defaultSize: 'md',
    defaultConfig: { thresholdDays: 30, includeNeverWorn: true, sortBy: 'days', topN: 8, range: 'all' },
    configSchema: [
      { key: 'thresholdDays', label: 'Days without wear', kind: 'number', min: 7, max: 365 },
      { key: 'includeNeverWorn', label: 'Include never worn', kind: 'toggle' },
      {
        key: 'sortBy',
        label: 'Sort by',
        kind: 'select',
        options: [
          { value: 'days', label: 'Longest unworn' },
          { value: 'price', label: 'Most expensive' },
          { value: 'recent', label: 'Most recent' },
        ],
      },
      { key: 'topN', label: 'Show top', kind: 'number', min: 3, max: 20 },
      rangeField(),
    ],
    component: DustyItems,
  },
  {
    type: 'day-of-week',
    title: 'Day of week',
    category: 'usage',
    description: 'Which weekdays you log outfits on.',
    defaultSize: 'sm',
    defaultConfig: { metric: 'outfits', range: 'all' },
    configSchema: [
      {
        key: 'metric',
        label: 'Count',
        kind: 'select',
        options: [
          { value: 'outfits', label: 'Outfits' },
          { value: 'items', label: 'Unique items' },
        ],
      },
      rangeField(),
    ],
    component: DayOfWeek,
  },
  {
    type: 'season-pattern',
    title: 'Seasonal pattern',
    category: 'usage',
    description: 'A year at a glance: month by weekday heatmap.',
    defaultSize: 'md',
    defaultConfig: { metric: 'outfits', range: 'all' },
    configSchema: [
      {
        key: 'metric',
        label: 'Count',
        kind: 'select',
        options: [
          { value: 'outfits', label: 'Outfits' },
          { value: 'items', label: 'Unique items' },
        ],
      },
      rangeField(),
    ],
    component: SeasonPattern,
  },

  // ---- Wardrobe composition ----
  {
    type: 'category-donut',
    title: 'By category',
    category: 'wardrobe',
    description: 'How your wardrobe is split across categories.',
    defaultSize: 'sm',
    defaultConfig: { level: 'category', metric: 'items', topN: 0 },
    configSchema: [
      {
        key: 'level',
        label: 'Level',
        kind: 'select',
        options: [
          { value: 'category', label: 'Category' },
          { value: 'subcategory', label: 'Subcategory' },
        ],
      },
      {
        key: 'metric',
        label: 'Value',
        kind: 'select',
        options: [
          { value: 'items', label: 'Item count' },
          { value: 'spend', label: 'Spend' },
        ],
      },
      { key: 'topN', label: 'Top N (0 = all)', kind: 'number', min: 0, max: 12 },
    ],
    component: CategoryDonut,
  },
  {
    type: 'color-bar',
    title: 'By color',
    category: 'wardrobe',
    description: 'Your color distribution, with real swatches.',
    defaultSize: 'md',
    defaultConfig: { metric: 'items', topN: 0 },
    configSchema: [
      {
        key: 'metric',
        label: 'Value',
        kind: 'select',
        options: [
          { value: 'items', label: 'Item count' },
          { value: 'spend', label: 'Spend' },
        ],
      },
      { key: 'topN', label: 'Top N (0 = all)', kind: 'number', min: 0, max: 15 },
    ],
    component: ColorBar,
  },
  {
    type: 'brand-bar',
    title: 'By brand',
    category: 'wardrobe',
    description: 'Which brands dominate your wardrobe.',
    defaultSize: 'md',
    defaultConfig: { topN: 8, metric: 'items', includeNoBrand: true },
    configSchema: [
      {
        key: 'metric',
        label: 'Value',
        kind: 'select',
        options: [
          { value: 'items', label: 'Item count' },
          { value: 'spend', label: 'Spend' },
          { value: 'avg', label: 'Avg price' },
        ],
      },
      { key: 'topN', label: 'Show top', kind: 'number', min: 3, max: 20 },
      { key: 'includeNoBrand', label: 'Include no brand', kind: 'toggle' },
    ],
    component: BrandBar,
  },
  {
    type: 'price-histogram',
    title: 'Price spread',
    category: 'wardrobe',
    description: 'How your purchase prices are distributed.',
    defaultSize: 'md',
    defaultConfig: { buckets: 8 },
    configSchema: [{ key: 'buckets', label: 'Buckets', kind: 'number', min: 4, max: 12 }],
    component: PriceHistogram,
  },
  {
    type: 'wardrobe-age',
    title: 'Wardrobe age',
    category: 'wardrobe',
    description: 'When you acquired your items, by year.',
    defaultSize: 'md',
    defaultConfig: { metric: 'items' },
    configSchema: [
      {
        key: 'metric',
        label: 'Value',
        kind: 'select',
        options: [
          { value: 'items', label: 'Item count' },
          { value: 'spend', label: 'Spend' },
        ],
      },
    ],
    component: WardrobeAge,
  },

  // ---- Value & cost ----
  {
    type: 'spend-stats',
    title: 'Spend',
    category: 'value',
    description: 'Total, average and median spend on your wardrobe.',
    defaultSize: 'sm',
    defaultConfig: { scope: 'all' },
    configSchema: [
      {
        key: 'scope',
        label: 'Scope',
        kind: 'select',
        options: [
          { value: 'all', label: 'All items' },
          { value: 'this_year', label: 'Acquired this year' },
          { value: 'last_year', label: 'Acquired last year' },
        ],
      },
    ],
    component: SpendStats,
  },
  {
    type: 'cost-per-wear',
    title: 'Cost per wear',
    category: 'value',
    description: 'Price divided by wears — the true cost of each item.',
    defaultSize: 'md',
    defaultConfig: {
      direction: 'best',
      topN: 5,
      focusItem: '',
      minWears: 0,
      source: 'best',
      showAverage: true,
      range: 'all',
    },
    configSchema: [
      { key: 'focusItem', label: 'Focus item', kind: 'item-select' },
      {
        key: 'direction',
        label: 'Rank',
        kind: 'select',
        options: [
          { value: 'best', label: 'Best value first' },
          { value: 'worst', label: 'Worst value first' },
        ],
      },
      {
        key: 'source',
        label: 'Wear source',
        kind: 'select',
        options: [
          { value: 'best', label: 'Best estimate' },
          { value: 'outfits', label: 'Logged outfits' },
          { value: 'wear_count', label: 'Wear count field' },
        ],
      },
      { key: 'minWears', label: 'Min wears', kind: 'number', min: 0, max: 20 },
      { key: 'topN', label: 'Show top', kind: 'number', min: 3, max: 15 },
      { key: 'showAverage', label: 'Show wardrobe average', kind: 'toggle' },
      rangeField(),
    ],
    component: CostPerWear,
  },
  {
    type: 'expensive-dusty',
    title: 'Expensive & unworn',
    category: 'value',
    description: 'Pricy items that are sitting in the closet.',
    defaultSize: 'md',
    defaultConfig: { minPrice: 50, maxWears: 0, sortBy: 'price', topN: 8 },
    configSchema: [
      { key: 'minPrice', label: 'Min price', kind: 'number', min: 0, max: 1000, step: 5 },
      { key: 'maxWears', label: 'Max wears', kind: 'number', min: 0, max: 20 },
      {
        key: 'sortBy',
        label: 'Sort by',
        kind: 'select',
        options: [
          { value: 'price', label: 'Most expensive' },
          { value: 'wears', label: 'Fewest wears' },
          { value: 'cpw', label: 'Worst cost per wear' },
        ],
      },
      { key: 'topN', label: 'Show top', kind: 'number', min: 3, max: 20 },
    ],
    component: ExpensiveDusty,
  },

  // ---- Sustainability ----
  {
    type: 'laundry-mix',
    title: 'Laundry impact',
    category: 'sustainability',
    description: 'How your wardrobe splits by laundry impact.',
    defaultSize: 'sm',
    defaultConfig: { metric: 'items', includeNotSet: true },
    configSchema: [
      {
        key: 'metric',
        label: 'Value',
        kind: 'select',
        options: [
          { value: 'items', label: 'Item count' },
          { value: 'spend', label: 'Spend' },
        ],
      },
      { key: 'includeNotSet', label: 'Include not set', kind: 'toggle' },
    ],
    component: LaundryMix,
  },
  {
    type: 'impact-hotspots',
    title: 'Impact hotspots',
    category: 'sustainability',
    description: 'High-impact items you wear the most.',
    defaultSize: 'md',
    defaultConfig: { topN: 5, includeMedium: false, minWears: 0 },
    configSchema: [
      { key: 'topN', label: 'Show top', kind: 'number', min: 3, max: 15 },
      { key: 'includeMedium', label: 'Include medium impact', kind: 'toggle' },
      { key: 'minWears', label: 'Min wears', kind: 'number', min: 0, max: 20 },
    ],
    component: ImpactHotspots,
  },

  // ---- Relationships ----
  {
    type: 'item-pairings',
    title: 'Item pairings',
    category: 'relationships',
    description: 'Which items you wear together most often.',
    defaultSize: 'md',
    defaultConfig: { topN: 5, focusItem: '', minCount: 1, range: 'all' },
    configSchema: [
      { key: 'focusItem', label: 'Focus item', kind: 'item-select' },
      { key: 'minCount', label: 'Min times together', kind: 'number', min: 1, max: 20 },
      { key: 'topN', label: 'Show top', kind: 'number', min: 3, max: 15 },
      rangeField(),
    ],
    component: ItemPairings,
  },
  {
    type: 'repeat-wears',
    title: 'Quick re-wears',
    category: 'relationships',
    description: 'Items you wear again within a few days.',
    defaultSize: 'md',
    defaultConfig: { windowDays: 7, topN: 8, range: 'all' },
    configSchema: [
      { key: 'windowDays', label: 'Window (days)', kind: 'number', min: 2, max: 30 },
      { key: 'topN', label: 'Show top', kind: 'number', min: 3, max: 15 },
      rangeField(),
    ],
    component: RepeatWears,
  },

  // ---- Insights ----
  {
    type: 'insights-callout',
    title: 'Insights',
    category: 'insights',
    description: 'Auto-generated observations about your habits.',
    defaultSize: 'md',
    defaultConfig: { focus: 'all', maxInsights: 6 },
    configSchema: [
      {
        key: 'focus',
        label: 'Focus',
        kind: 'select',
        options: [
          { value: 'all', label: 'All' },
          { value: 'usage', label: 'Usage' },
          { value: 'value', label: 'Value' },
          { value: 'sustainability', label: 'Sustainability' },
        ],
      },
      { key: 'maxInsights', label: 'Max insights', kind: 'number', min: 3, max: 12 },
    ],
    component: InsightsCallout,
  },
];

export const WIDGET_CATALOG: Map<string, WidgetDef> = new Map(defs.map(d => [d.type, d]));
