import { expect, test } from '@playwright/test';
import { login, trackConsoleErrors } from './helpers';

test('renders the insights widgets over seeded data without console errors', async ({ page }) => {
  const errors = trackConsoleErrors(page);

  await login(page);
  await page.getByRole('button', { name: 'Insights' }).click();

  // The view finished loading (the stub has items and outfits to chart).
  await page.getByRole('heading', { name: 'Insights', level: 2 }).waitFor();

  // The default layout is eight widgets; they all mount.
  await expect(page.locator('.widget-card')).toHaveCount(8);

  // Stat cards show the seeded wardrobe, and at least one canvas chart drew.
  await expect(page.locator('.widget-card canvas').first()).toBeVisible();
  await expect(page.locator('.widget-card', { hasText: 'Items' }).first()).toBeVisible();

  expect(errors, `console errors: ${errors.join(' | ')}`).toEqual([]);
});
