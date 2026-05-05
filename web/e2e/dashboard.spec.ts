import { test, expect } from '@playwright/test';
import { injectAuth, injectConnectedEventSource, injectFailingEventSource, mockApiRoutes } from './helpers';

test.describe('Dashboard — realtime stream status', () => {
	test('shows a stream status pill in the header', async ({ page }) => {
		await mockApiRoutes(page);
		await injectAuth(page);
		// No EventSource mock → streams start in 'connecting' state (no real backend)
		await page.goto('/dashboard');
		const pill = page.locator('.section-header .pill').first();
		await expect(pill).toBeVisible();
		await expect(pill).toHaveText('connecting');
	});

	test('stream pill shows "connected" after SSE fires connected event', async ({ page }) => {
		await mockApiRoutes(page);
		await injectConnectedEventSource(page); // mock EventSource fires onopen then connected event
		await injectAuth(page);

		await page.goto('/dashboard');
		await expect(page.locator('.section-header .pill').first()).toHaveText('connected', {
			timeout: 8_000
		});
	});

	test('shows degraded banner when all SSE streams report errors', async ({ page }) => {
		await mockApiRoutes(page);
		await injectFailingEventSource(page); // mock EventSource fires onerror immediately
		await injectAuth(page);

		await page.goto('/dashboard');

		// After onerror, scheduleReconnect sets state to 'reconnecting' → degraded banner appears
		await expect(page.locator('.degraded-banner')).toBeVisible({ timeout: 10_000 });
	});

	test('renders activity panel headings', async ({ page }) => {
		await mockApiRoutes(page);
		await injectAuth(page);

		await page.goto('/dashboard');
		await expect(page.getByRole('heading', { name: 'Download Queue' })).toBeVisible();
		await expect(page.getByRole('heading', { name: 'Import Processing' })).toBeVisible();
		await expect(page.getByRole('heading', { name: 'Scheduled Tasks' })).toBeVisible();
	});

	test('renders empty-state messages for idle panels', async ({ page }) => {
		await mockApiRoutes(page);
		await injectAuth(page);

		await page.goto('/dashboard');
		await expect(page.getByText('Download queue is idle')).toBeVisible({ timeout: 8_000 });
	});
});
