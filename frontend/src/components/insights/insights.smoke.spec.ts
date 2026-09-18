// @vitest-environment happy-dom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';

// Chart.js needs a real canvas 2d context; stub the wrapper for DOM tests.
vi.mock('vue-chartjs', async () => {
  const { defineComponent, h } = await vi.importActual<typeof import('vue')>('vue');
  return {
    Chart: defineComponent({
      name: 'MockChart',
      props: ['type', 'data', 'options'],
      render: () => h('div', { class: 'mock-chart' }),
    }),
  };
});

vi.mock('../../api/client', () => ({
  api: {
    listClothingItems: vi.fn().mockResolvedValue([
      {
        id: 'i1',
        name: 'Blue Shirt',
        category_id: 1,
        subcategory_id: null,
        color_id: 'blue',
        brand_id: 'b1',
        purchase_price: 30,
        owned_since: '2024-01-01',
        laundry_impact: 'low',
        is_active: true,
        wear_count: 3,
        image_path: null,
        created_at: '2024-01-01T00:00:00Z',
        updated_at: '2024-01-01T00:00:00Z',
      },
      {
        id: 'i2',
        name: 'Dark Jeans',
        category_id: 2,
        subcategory_id: null,
        color_id: null,
        brand_id: null,
        purchase_price: null,
        owned_since: null,
        laundry_impact: null,
        is_active: false,
        wear_count: null,
        image_path: null,
        created_at: '2024-06-01T00:00:00Z',
        updated_at: '2024-06-01T00:00:00Z',
      },
    ]),
    listOutfits: vi.fn().mockResolvedValue([
      {
        id: 'o1',
        date: new Date(Date.now() - 86400000).toISOString(),
        notes: null,
        image_path: null,
        is_active: true,
        created_at: new Date().toISOString(),
        items: [
          { id: 'i1', name: 'Blue Shirt', image_path: null, color_id: 'blue' },
          { id: 'i2', name: 'Dark Jeans', image_path: null, color_id: null },
        ],
      },
    ]),
    listColors: vi.fn().mockResolvedValue([{ id: 'blue', hex_value: '#0000ff' }]),
    listCategories: vi.fn().mockResolvedValue([
      { id: 1, name: 'Tops' },
      { id: 2, name: 'Bottoms' },
    ]),
    listSubcategories: vi.fn().mockResolvedValue([]),
    listBrands: vi.fn().mockResolvedValue([{ id: 'b1', name: 'Acme' }]),
  },
  imageUrl: vi.fn().mockReturnValue(null),
}));

import InsightsView from '../InsightsView.vue';

async function mountView() {
  const wrapper = mount(InsightsView);
  await flushPromises();
  return wrapper;
}

beforeEach(() => {
  localStorage.clear();
});

describe('InsightsView smoke', () => {
  it('renders the default layout after loading', async () => {
    const wrapper = await mountView();
    expect(wrapper.find('.loading').exists()).toBe(false);
    expect(wrapper.findAll('.widget-card')).toHaveLength(8);
    expect(wrapper.find('.add-tile').exists()).toBe(true);
    expect(wrapper.find('.mock-chart').exists()).toBe(true);
  });

  it('persists layout changes to localStorage (but not the untouched default)', async () => {
    const wrapper = await mountView();
    expect(localStorage.getItem('insights.layout.v1')).toBeNull();

    wrapper.find('.insights-actions .md-btn--tonal').trigger('click');
    await flushPromises();
    wrapper
      .findAll('.palette__item')
      .find(b => b.text().includes('Laundry impact'))!
      .trigger('click');
    await flushPromises();

    const parsed = JSON.parse(localStorage.getItem('insights.layout.v1') as string);
    expect(parsed.version).toBe(1);
    expect(parsed.widgets).toHaveLength(9);
  });

  it('adds a widget from the palette', async () => {
    const wrapper = await mountView();
    wrapper.find('.insights-actions .md-btn--tonal').trigger('click');
    await flushPromises();
    expect(wrapper.find('.palette').exists()).toBe(true);

    const laundryItem = wrapper
      .findAll('.palette__item')
      .find(b => b.text().includes('Laundry impact'));
    expect(laundryItem).toBeTruthy();
    laundryItem!.trigger('click');
    await flushPromises();

    expect(wrapper.find('.palette').exists()).toBe(false);
    expect(wrapper.findAll('.widget-card')).toHaveLength(9);
    const raw = JSON.parse(localStorage.getItem('insights.layout.v1') as string);
    expect(raw.widgets.map((w: { type: string }) => w.type)).toContain('laundry-mix');
  });

  it('removes a widget from its menu', async () => {
    const wrapper = await mountView();
    const first = wrapper.findAll('.widget-card')[0];
    first.find('.widget-card__menu-btn').trigger('click');
    await flushPromises();
    first
      .findAll('.widget-card__menu-panel button')
      .find(b => b.text().includes('Remove'))!
      .trigger('click');
    await flushPromises();
    expect(wrapper.findAll('.widget-card')).toHaveLength(7);
  });

  it('reorders widgets via drag and drop', async () => {
    const wrapper = await mountView();
    const types = () => wrapper.findAll('.widget-card').map(c => c.attributes('data-widget-type'));
    const before = types();
    expect(before.slice(0, 5)).toEqual(['stat-card', 'stat-card', 'stat-card', 'wear-trend', 'category-donut']);

    const cards = () => wrapper.findAll('.widget-card');
    cards()[0].find('.widget-card__handle').trigger('mousedown');
    cards()[0].trigger('dragstart');
    cards()[4].trigger('dragover');
    cards()[4].trigger('drop');
    cards()[0].trigger('dragend');
    await flushPromises();

    // card 0 moved to index 4, shifting 1..3 down
    expect(types()).toEqual(['stat-card', 'stat-card', 'wear-trend', 'category-donut', 'stat-card', ...before.slice(5)]);
  });

  it('resizes a widget from its config panel', async () => {
    const wrapper = await mountView();
    const first = wrapper.findAll('.widget-card')[0];
    first.find('.widget-card__menu-btn').trigger('click');
    await flushPromises();
    first
      .findAll('.widget-card__menu-panel button')
      .find(b => b.text().includes('Configure'))!
      .trigger('click');
    await flushPromises();

    expect(first.find('.widget-card__config').exists()).toBe(true);
    first
      .findAll('.size-picker__btn')
      .find(b => b.text() === 'LG')!
      .trigger('click');
    await flushPromises();
    expect(wrapper.findAll('.widget-card')[0].classes()).toContain('widget-card--lg');
  });

  it('filters items with the category filter', async () => {
    const wrapper = await mountView();
    const firstStat = wrapper.findAll('.widget-card')[0].find('.stat__value');
    expect(firstStat.text()).toBe('2');

    const catBtn = wrapper
      .findAll('.fselect__btn')
      .find(b => b.text().includes('Category'))!;
    catBtn.trigger('click');
    await flushPromises();
    wrapper.find('.fselect__panel input[type="checkbox"]').trigger('change');
    await flushPromises();

    expect(wrapper.findAll('.widget-card')[0].find('.stat__value').text()).toBe('1');
  });

  it('shows generated insight bullets', async () => {
    const wrapper = await mountView();
    const callout = wrapper
      .findAll('.widget-card')
      .find(c => c.find('.widget-card__title').text() === 'Insights');
    expect(callout).toBeTruthy();
    expect(callout!.findAll('.callout__item').length).toBeGreaterThan(0);
  });
});
