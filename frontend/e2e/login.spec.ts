import { expect, test } from '@playwright/test';
import { login, trackConsoleErrors, E2E_EMAIL, E2E_PASSWORD } from './helpers';

test('logs in with email and password and reaches the app shell', async ({ page }) => {
  const errors = trackConsoleErrors(page);

  await login(page);

  // The shell shows the signed-in email and lands on the home tab (Outfits).
  await expect(page.getByRole('heading', { name: 'Cloth Web' })).toBeVisible();
  await expect(page.locator('.month-label')).toBeVisible();

  expect(errors, `console errors: ${errors.join(' | ')}`).toEqual([]);
});

test('shows the server error for wrong credentials', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('heading', { name: 'Sign in' }).waitFor();
  await page.getByLabel('Email').fill(E2E_EMAIL);
  await page.getByLabel('Password').fill('definitely-wrong');
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();

  await expect(page.locator('.error')).toHaveText('Invalid login credentials');
});
