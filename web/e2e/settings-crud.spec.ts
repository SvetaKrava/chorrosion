import { expect, test, type Page } from '@playwright/test';
import { injectAuth, mockApiRoutes } from './helpers';
import {
	mockDownloadClientsCrud,
	mockIndexersCrud,
	mockMetadataProfilesCrud,
	mockQualityProfilesCrud
} from './settings-fixtures';

test.describe('Settings CRUD', () => {
	test.beforeEach(async ({ page }) => {
		await mockApiRoutes(page);
		await injectAuth(page);
	});

	async function openSettingsSubpage(page: Page, linkName: string) {
		const slugs: Record<string, string> = {
			'Download Clients': '/settings/download-clients',
			Indexers: '/settings/indexers',
			'Quality Profiles': '/settings/quality-profiles',
			'Metadata Profiles': '/settings/metadata-profiles'
		};
		await page.goto(slugs[linkName]);
		await expect(page).toHaveURL(slugs[linkName]);
		await expect(page.getByRole('link', { name: linkName })).toHaveAttribute('aria-current', 'page');
		await expect(page.getByRole('heading', { name: linkName })).toBeVisible();
	}

	test('download clients CRUD round trip', async ({ page }) => {
		await mockDownloadClientsCrud(page);
		await openSettingsSubpage(page, 'Download Clients');

		await expect(page.getByText('Seed Client')).toBeVisible();

		await page.getByRole('button', { name: 'Add Client' }).click();
		await page.getByLabel('Name *').fill('Created Client');
		await page.getByLabel('Base URL *').fill('http://localhost:8081');
		await page.getByRole('button', { name: 'Add Client' }).last().click();
		await expect(page.getByText('Created Client')).toBeVisible();

		const row = page.locator('.client-item').filter({ hasText: 'Created Client' });
		await row.getByRole('button', { name: /Edit/ }).click();
		await page.getByLabel('Name *').fill('Updated Client');
		await page.getByRole('button', { name: 'Save Changes' }).click();
		await expect(page.getByText('Updated Client')).toBeVisible();

		const updatedRow = page.locator('.client-item').filter({ hasText: 'Updated Client' });
		await updatedRow.getByRole('button', { name: /Delete/ }).click();
		const dialog = page.getByRole('dialog');
		await dialog.getByRole('button', { name: 'Delete' }).click();
		await expect(updatedRow).toHaveCount(0);
	});

	test('indexers CRUD round trip', async ({ page }) => {
		await mockIndexersCrud(page);
		await openSettingsSubpage(page, 'Indexers');

		await expect(page.getByText('Seed Indexer')).toBeVisible();

		await page.getByRole('button', { name: 'Add Indexer' }).click();
		await page.getByLabel('Name *').fill('Created Indexer');
		await page.getByLabel('Base URL *').fill('http://localhost:9118');
		await page.getByRole('button', { name: 'Add Indexer' }).last().click();
		await expect(page.getByText('Created Indexer')).toBeVisible();

		const row = page.locator('.indexer-item').filter({ hasText: 'Created Indexer' });
		await row.getByRole('button', { name: /Edit/ }).click();
		await page.getByLabel('Name *').fill('Updated Indexer');
		await page.getByRole('button', { name: 'Save Changes' }).click();
		await expect(page.getByText('Updated Indexer')).toBeVisible();

		const updatedRow = page.locator('.indexer-item').filter({ hasText: 'Updated Indexer' });
		await updatedRow.getByRole('button', { name: /Delete/ }).click();
		await page.getByRole('dialog').getByRole('button', { name: 'Delete' }).click();
		await expect(updatedRow).toHaveCount(0);
	});

	test('quality profiles CRUD round trip', async ({ page }) => {
		await mockQualityProfilesCrud(page);
		await openSettingsSubpage(page, 'Quality Profiles');

		await expect(page.getByText('Seed Quality')).toBeVisible();

		await page.getByRole('button', { name: 'Add Profile' }).click();
		await page.getByLabel('Name *').fill('Created Quality');
		await page.getByRole('button', { name: 'Add Profile' }).last().click();
		await expect(page.getByText('Created Quality')).toBeVisible();

		const row = page.locator('.profile-item').filter({ hasText: 'Created Quality' });
		await row.getByRole('button', { name: /Edit/ }).click();
		await page.getByLabel('Name *').fill('Updated Quality');
		await page.getByRole('button', { name: 'Save Changes' }).click();
		await expect(page.getByText('Updated Quality')).toBeVisible();

		const updatedRow = page.locator('.profile-item').filter({ hasText: 'Updated Quality' });
		await updatedRow.getByRole('button', { name: /Delete/ }).click();
		await page.getByRole('dialog').getByRole('button', { name: 'Delete' }).click();
		await expect(updatedRow).toHaveCount(0);
	});

	test('metadata profiles CRUD round trip', async ({ page }) => {
		await mockMetadataProfilesCrud(page);
		await openSettingsSubpage(page, 'Metadata Profiles');

		await expect(page.getByText('Seed Metadata')).toBeVisible();

		await page.getByRole('button', { name: 'Add Profile' }).click();
		await page.getByLabel('Name *').fill('Created Metadata');
		await page.getByRole('button', { name: 'Add Profile' }).last().click();
		await expect(page.getByText('Created Metadata')).toBeVisible();

		const row = page.locator('.profile-item').filter({ hasText: 'Created Metadata' });
		await row.getByRole('button', { name: /Edit/ }).click();
		await page.getByLabel('Name *').fill('Updated Metadata');
		await page.getByRole('button', { name: 'Save Changes' }).click();
		await expect(page.getByText('Updated Metadata')).toBeVisible();

		const updatedRow = page.locator('.profile-item').filter({ hasText: 'Updated Metadata' });
		await updatedRow.getByRole('button', { name: /Delete/ }).click();
		await page.getByRole('dialog').getByRole('button', { name: 'Delete' }).click();
		await expect(updatedRow).toHaveCount(0);
	});
});
