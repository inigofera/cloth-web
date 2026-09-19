// @vitest-environment happy-dom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';

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

function daysAgoIso(n: number): string {
  return new Date(Date.now() - n * 86400000).toISOString();
}

vi.mock('../../api/client', () => {
  const a = { id: 'i1', name: 'Blue Shirt', category_id: 1, subcategory_id: null, color_id: 'blue', brand_id: 'b1', purchase_price: 30, owned_since: '2024-01-01', laundry_impact: 'low', is_active: true, wear_count: null, image_path: null, created_at: '2024-01-01T00:00:00Z', updated_at: '2024-01-01T00:00:00Z' };
  const b = { id: 'i2', name: 'Dark Jeans', category_id: 2, subcategory_id: null, color_id: null, brand_id: null, purchase_price: null, owned_since: null, laundry_impact: null, is_active: true, wear_count: null, image_path: null, created_at: '2024-06-01T00:00:00Z', updated_at: '2024-06-01T00:00:00Z' };
  return {
    api: {
      listClothingItems: vi.fn().mockResolvedValue([a, b]),
      listOutfits: vi.fn().mockResolvedValue([
        { id: 'o-recent', date: daysAgoIso(2), notes: null, image_path: null, is_active: true, created_at: daysAgoIso(2), items: [{ id: 'i1', name: 'Blue Shirt', image_path: null, color_id: 'blue' }] },
        { id: 'o-old', date: daysAgoIso(200), notes: null, image_path: null, is_active: true, created_at: daysAgoIso(200), items: [{ id: 'i2', name: 'Dark Jeans', image_path: null, color_id: null }] },
      ]),
      listColors: vi.fn().mockResolvedValue([{ id: 'blue', hex_value: '#0000ff' }]),
      listCategories: vi.fn().mockResolvedValue([{ id: 1, name: 'Tops' }, { id: 2, name: 'Bottoms' }]),
      listSubcategories: vi.fn().mockResolvedValue([]),
      listBrands: vi.fn().mockResolvedValue([{ id: 'b1', name: 'Acme' }]),
    },
    imageUrl: vi.fn().mockReturnValue(null),
  };
});

import InsightsView from '../InsightsView.vue';

async function mountView() {
  const wrapper = mount(InsightsView);
  await flushPromises();
  return wrapper;
}

beforeEach(() => {
  localStorage.clear();
});

describe('global Period filter', () => {
  it('narrows usage widgets to the selected period', async () => {
    const wrapper = await mountView();

    // The "Outfits logged" stat card (2nd in default layout) should show 2 (all time).
    const statCards = wrapper.findAll('.widget-card').filter(c => c.find('.stat__value').exists());
    const outfitsStat = statCards.find(c => c.find('.stat__label').text() === 'Outfits logged');
    expect(outfitsStat).toBeTruthy();
    expect(outfitsStat!.find('.stat__value').text()).toBe('2');

    // Open the global Period filter and choose "Last 30 days".
    const periodBtn = wrapper
      .findAll('.fselect__btn')
      .find(b => b.text().includes('Period'))!;
    periodBtn.trigger('click');
    await flushPromises();
    wrapper
      .findAll('.fselect__option')
      .find(o => o.text().includes('Last 30 days'))!
      .trigger('click');
    await flushPromises();

    // Now only the recent outfit should count → 1.
    expect(outfitsStat!.find('.stat__value').text()).toBe('1');
  });
});
