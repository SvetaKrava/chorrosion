import { test, expect } from '@playwright/test';
import { injectAuth, mockApiRoutes, MOCK_USER, API_BASE } from './helpers';

test.describe('Authentication', () => {
	test('shows the login form at the root path', async ({ page }) => {
		await page.goto('/');
		await expect(page.getByRole('heading', { name: 'Chorrosion Control Deck' })).toBeVisible();
		await expect(page.getByRole('heading', { name: 'Sign In' })).toBeVisible();
		await expect(page.getByLabel('Username')).toBeVisible();
		await expect(page.getByLabel('Password')).toBeVisible();
		await expect(page.getByRole('button', { name: 'Sign In' })).toBeVisible();
	});

	test('shows an error message on failed login', async ({ page }) => {
		// Override the default stub to return 401
		await page.route(`${API_BASE}/api/v1/auth/forms/login`, (route) =>
			route.fulfill({
				status: 401,
				contentType: 'application/json',
				body: JSON.stringify({ error: 'Invalid credentials' })
			})
		);

		await page.goto('/');
		await page.getByLabel('Username').fill('baduser');
		await page.getByLabel('Password').fill('wrongpass');
		await page.getByRole('button', { name: 'Sign In' }).click();

		await expect(page.getByText('Invalid credentials')).toBeVisible();
		// Should remain on the login page
		await expect(page).toHaveURL('/');
	});

	test('redirects to /dashboard after successful login', async ({ page }) => {
		await mockApiRoutes(page);

		await page.goto('/');
		await page.getByLabel('Username').fill('testuser');
		await page.getByLabel('Password').fill('testpass');
		await page.getByRole('button', { name: 'Sign In' }).click();

		await expect(page).toHaveURL('/dashboard');
		await expect(page.getByRole('heading', { name: 'Dashboard' })).toBeVisible();
	});

	test('logs out and returns to the login page', async ({ page }) => {
		await mockApiRoutes(page);
		await injectAuth(page);

		await page.goto('/dashboard');
		await expect(
			page.getByRole('button', { name: /Log Out/i })
		).toBeVisible();

		await page.getByRole('button', { name: /Log Out/i }).click();

		await expect(page).toHaveURL('/');
		await expect(page.getByRole('heading', { name: 'Sign In' })).toBeVisible();
	});

	test('displays logged-in username in the header', async ({ page }) => {
		await mockApiRoutes(page);
		await injectAuth(page);

		await page.goto('/dashboard');
		// Username is rendered in a <span> with aria-label
		await expect(
			page.locator(`[aria-label="Logged in as ${MOCK_USER.username}"]`)
		).toBeVisible();
	});
});
