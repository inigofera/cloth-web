import { expect, test } from '@playwright/test';
import { login, trackConsoleErrors } from './helpers';

const API_KEY = 'AIzaSy-e2e-stub-key-1234';

test('switches the try-on provider, saves settings, and persists after reload', async ({ page }, testInfo) => {
  // The stub's settings persist across retries of this test, and Save URL is
  // disabled when the input equals the saved value — so pick a fresh URL per
  // attempt.
  const QWEN_URL = `http://127.0.0.1:${9990 + testInfo.retry}`;
  const errors = trackConsoleErrors(page);
  // The toast's accessible text includes its close button; assert the message span.
  const toast = page.getByRole('status').locator('.toast-message');

  await login(page);
  await page.getByRole('button', { name: 'Settings' }).click();
  await page.getByRole('heading', { name: 'Virtual Try-On Settings' }).waitFor();

  // Ensure the Gemini panel is active. A fresh stub starts on Gemini; a retry
  // inherits the provider the previous attempt left behind, so click the
  // Gemini card either way (a no-op when already active).
  await page.locator('.engine-card', { hasText: 'Nano Banana' }).click();
  await expect(page.locator('.subpanel-title', { hasText: 'Gemini API Key' })).toBeVisible();

  // Switch to the local Qwen engine: PUT /settings + toast.
  await page.locator('.engine-card', { hasText: 'Qwen-Image-2.1-Turbo' }).click();
  await expect(toast).toHaveText('Try-on engine switched to Local Qwen-Image.');
  await expect(
    page.locator('.subpanel-title', { hasText: 'Local Server Configuration' }),
  ).toBeVisible();

  // Save a custom Qwen URL.
  await page.getByLabel('Local Qwen server URL').fill(QWEN_URL);
  await page.getByRole('button', { name: 'Save URL' }).click();
  await expect(toast).toHaveText('Local server URL saved.');

  // Switch back to Gemini and store an API key.
  await page.locator('.engine-card', { hasText: 'Nano Banana' }).click();
  await expect(toast).toHaveText('Try-on engine switched to Nano Banana (Gemini API).');
  await page.getByLabel('Gemini API key').fill(API_KEY);
  await page.getByRole('button', { name: 'Save Key' }).click();
  await expect(toast).toHaveText('Gemini API key saved securely.');
  // Only the hint is exposed, never the key itself.
  await expect(page.locator('.status-badge', { hasText: '...1234' }).first()).toBeVisible();

  // A reload reloads settings from the API: provider, key hint and Qwen URL
  // must all survive.
  await page.reload();
  await page.getByRole('button', { name: 'Settings' }).click();
  await page.getByRole('heading', { name: 'Virtual Try-On Settings' }).waitFor();

  await expect(page.locator('.engine-card', { hasText: 'Nano Banana' })).toHaveClass(
    /engine-card--active/,
  );
  await expect(page.locator('.status-badge', { hasText: '...1234' }).first()).toBeVisible();

  // Flip to Qwen again and confirm the saved URL came back.
  await page.locator('.engine-card', { hasText: 'Qwen-Image-2.1-Turbo' }).click();
  await expect(page.getByLabel('Local Qwen server URL')).toHaveValue(QWEN_URL);

  expect(errors, `console errors: ${errors.join(' | ')}`).toEqual([]);
});
