import { test, expect } from '@playwright/test';
import { injectAuth, mockApiRoutes } from './helpers';

test.describe('Navigation', () => {
	test.beforeEach(async ({ page }) => {
		await mockApiRoutes(page);
		await injectAuth(page);
	});

	test('renders all four nav links in the header', async ({ page }) => {
		await page.goto('/dashboard');
		const nav = page.getByRole('navigation', { name: 'Main navigation' });
		await expect(nav.getByRole('link', { name: 'Dashboard' })).toBeVisible();
		await expect(nav.getByRole('link', { name: 'Artists' })).toBeVisible();
		await expect(nav.getByRole('link', { name: 'Albums' })).toBeVisible();
		await expect(nav.getByRole('link', { name: 'Appearance' })).toBeVisible();
	});

	test('active nav link has aria-current="page"', async ({ page }) => {
		await page.goto('/dashboard');
		const link = page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Dashboard' });
		await expect(link).toHaveAttribute('aria-current', 'page');
	});

	test('non-active nav links do not have aria-current', async ({ page }) => {
		await page.goto('/dashboard');
		const nav = page.getByRole('navigation', { name: 'Main navigation' });
		for (const name of ['Artists', 'Albums', 'Appearance']) {
			await expect(nav.getByRole('link', { name })).not.toHaveAttribute('aria-current');
		}
	});

	test('clicking Artists navigates to /artists', async ({ page }) => {
		await page.goto('/dashboard');
		await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Artists' }).click();
		await expect(page).toHaveURL('/artists');
		await expect(page.getByRole('heading', { name: 'Artists' })).toBeVisible();
	});

	test('clicking Albums navigates to /albums', async ({ page }) => {
		await page.goto('/dashboard');
		await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Albums' }).click();
		await expect(page).toHaveURL('/albums');
		await expect(page.getByRole('heading', { name: 'Albums' })).toBeVisible();
	});

	test('clicking Appearance navigates to /appearance', async ({ page }) => {
		await page.goto('/dashboard');
		await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Appearance' }).click();
		await expect(page).toHaveURL('/appearance');
	});

	test('nav link aria-current tracks the active page', async ({ page }) => {
		await page.goto('/artists');
		const nav = page.getByRole('navigation', { name: 'Main navigation' });
		await expect(nav.getByRole('link', { name: 'Artists' })).toHaveAttribute('aria-current', 'page');
		await expect(nav.getByRole('link', { name: 'Dashboard' })).not.toHaveAttribute('aria-current');
	});

	test('skip link is present and points to main content', async ({ page }) => {
		await page.goto('/dashboard');
		const skipLink = page.getByRole('link', { name: 'Skip to main content' });
		// Visually hidden until focused (off-screen via CSS) but present in DOM
		await expect(skipLink).toBeAttached();
		await expect(skipLink).toHaveAttribute('href', '#main-content');
	});
});
