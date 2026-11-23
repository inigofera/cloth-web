import { expect, test, type Page } from '@playwright/test';
import { login, trackConsoleErrors, TINY_PNG } from './helpers';

test('lists the wardrobe and adds an item with an image upload', async ({ page }, testInfo) => {
  const errors = trackConsoleErrors(page);
  // The stub's items persist across retries of this test, so make the name
  // unique per attempt.
  const NAME = `E2E Flannel ${testInfo.retry}`;
  const FILE_NAME = `e2e-flannel-${testInfo.retry}.png`;

  await login(page);
  await page.getByRole('button', { name: 'Clothes' }).click();

  // Seeded items render, with images.
  const seeded = page.locator('.item-card', { hasText: 'White Tee' });
  await expect(seeded).toBeVisible();
  await expect(seeded.locator('img.item-image')).toHaveAttribute('src', /\/api\/files\/users\//);

  // The seeded count, before our add.
  await expect(page.locator('.section-title')).toHaveText(/My clothes \(\d+\)/);
  const before = await countItems(page);

  const form = page.locator('.form-panel');
  await form.getByPlaceholder('e.g. Blue Denim Jacket').fill(NAME);
  // First field row is category/subcategory; the first select in it is category.
  await form
    .locator('.field-row')
    .first()
    .locator('select')
    .first()
    .selectOption({ label: 'Tops' });
  await form
    .locator('.field-row')
    .nth(2)
    .locator('input[type=number]')
    .first()
    .fill('39.50');
  await form
    .locator('input[type=file]')
    .setInputFiles({ name: FILE_NAME, mimeType: 'image/png', buffer: TINY_PNG });
  // The form acknowledges the file before submit.
  await expect(form.locator('.field-hint')).toContainText(FILE_NAME);

  await form.getByRole('button', { name: 'Add item' }).click();

  const card = page.locator('.item-card', { hasText: NAME });
  await expect(card).toBeVisible();
  // The uploaded image is served back from the stub's /files endpoint.
  await expect(card.locator('img.item-image')).toHaveAttribute('src', /\/api\/files\/users\//);
  await expect(page.locator('.section-title')).toHaveText(
    new RegExp(`My clothes \\(${before + 1}\\)`),
  );

  expect(errors, `console errors: ${errors.join(' | ')}`).toEqual([]);
});

async function countItems(page: Page): Promise<number> {
  return page.locator('.item-card').count();
}
