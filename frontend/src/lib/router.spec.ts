// @vitest-environment happy-dom
import { describe, it, expect, beforeEach } from 'vitest';
import { useRouter } from './router';

describe('useRouter', () => {
  beforeEach(() => {
    window.location.hash = '';
  });

  it('defaults to outfits when hash is empty', () => {
    const { currentTab } = useRouter();
    expect(currentTab.value).toBe('outfits');
  });

  it('updates tab and hash on navigate', () => {
    const router = useRouter();
    router.navigate('clothes');
    expect(router.currentTab.value).toBe('clothes');
    expect(window.location.hash).toBe('#clothes');
  });

  it('supports query parameters for deep linking', () => {
    const router = useRouter();
    router.navigate('clothes', { item: 'item-42' });
    expect(router.currentTab.value).toBe('clothes');
    expect(router.params.value.item).toBe('item-42');
    expect(window.location.hash).toContain('item=item-42');
  });
});
