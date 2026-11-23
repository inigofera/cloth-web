import { expect, test } from '@playwright/test';
import { login, trackConsoleErrors } from './helpers';

test('logs an outfit for today and shows it on the calendar', async ({ page }, testInfo) => {
  const errors = trackConsoleErrors(page);
  // The stub's outfits persist across retries of this test, so make the
  // notes unique per attempt (they double as the outfit's chip label).
  const NOTES = `e2e smoke outfit ${testInfo.retry}`;

  await login(page);
  // The home tab is Outfits: the calendar is already on screen.
  await page.getByRole('button', { name: "Log today's outfit" }).click();

  // The day panel opens on today's local date. Compute the expected label in
  // the page (same engine and locale as the app) instead of in node, so the
  // assertion never depends on host locale differences.
  const expectedTitle = await page.evaluate(() => {
    const d = new Date();
    const p = (n: number) => String(n).padStart(2, '0');
    const [y, m, day] = [d.getFullYear(), d.getMonth() + 1, d.getDate()];
    // Same options the app uses (formatSelectedDateLabel), computed in the
    // page so locale and engine match exactly.
    return new Date(y, m - 1, day).toLocaleDateString(undefined, {
      weekday: 'long',
      month: 'long',
      day: 'numeric',
      year: 'numeric',
    });
  });
  await expect(page.locator('.day-title')).toHaveText(expectedTitle);

  // Pick two seeded items from the picker and add notes.
  const picker = page.locator('.item-picker');
  await picker.locator('.item-option', { hasText: 'White Tee' }).getByRole('checkbox').check();
  await picker.locator('.item-option', { hasText: 'Blue Jeans' }).getByRole('checkbox').check();
  await expect(page.locator('.selected-chips')).toContainText('White Tee');
  await page.getByPlaceholder('optional').fill(NOTES);

  await page.getByRole('button', { name: 'Add outfit' }).click();

  // The outfit card appears in the day panel with its notes and both items.
  const card = page.locator('.outfit-card', { hasText: NOTES });
  await expect(card).toBeVisible();
  await expect(card.locator('.item-tag', { hasText: 'White Tee' })).toBeVisible();
  await expect(card.locator('.item-tag', { hasText: 'Blue Jeans' })).toBeVisible();

  // ...and the calendar's today cell carries the outfit chip.
  await expect(page.locator('.day-cell.today .outfit-chip')).toHaveCount(1);

  expect(errors, `console errors: ${errors.join(' | ')}`).toEqual([]);
});
