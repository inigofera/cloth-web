import { expect, type Page } from '@playwright/test';

// The single seeded user the auth stub knows.
export const E2E_EMAIL = 'e2e@example.com';
export const E2E_PASSWORD = 'e2e-password';

/** A small valid PNG (1x1) for upload steps. */
export const TINY_PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAAABJRUkErkJggg==',
  'base64',
);

/**
 * Collect console errors and uncaught page errors for the lifetime of the
 * page. The favicon 404 (vite has no favicon) is noise, not an app error.
 */
export function trackConsoleErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on('console', (msg) => {
    if (msg.type() !== 'error') return;
    const text = msg.text();
    if (text.includes('favicon')) return;
    errors.push(text);
  });
  page.on('pageerror', (err) => errors.push(err.message));
  return errors;
}

/**
 * Sign in through the real login form (email/password path) and wait for the
 * app shell. The default tab after login is Outfits (the router's home tab).
 */
export async function login(page: Page): Promise<void> {
  await page.goto('/');
  await page.getByRole('heading', { name: 'Sign in' }).waitFor();
  await page.getByLabel('Email').fill(E2E_EMAIL);
  await page.getByLabel('Password').fill(E2E_PASSWORD);
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await page.locator('.who').waitFor();
  await expect(page.locator('.who')).toHaveText(E2E_EMAIL);
}
