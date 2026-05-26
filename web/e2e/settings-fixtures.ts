import type { Page, Route } from '@playwright/test';

type DownloadClient = {
	id: string;
	name: string;
	client_type: string;
	base_url: string;
	username: string | null;
	category: string | null;
	enabled: boolean;
	has_password: boolean;
};

type Indexer = {
	id: string;
	name: string;
	base_url: string;
	protocol: string;
	enabled: boolean;
	has_api_key: boolean;
};

type QualityProfile = {
	id: string;
	name: string;
	allowed_qualities: string[];
	upgrade_allowed: boolean;
	cutoff_quality: string | null;
};

type MetadataProfile = {
	id: string;
	name: string;
	primary_album_types: string[];
	secondary_album_types: string[];
	release_statuses: string[];
};

function routeJson(page: Page, glob: string, handler: (route: Route) => Promise<void> | void) {
	return page.route(glob, handler);
}

export async function mockDownloadClientsCrud(page: Page): Promise<void> {
	let nextId = 2;
	const items: DownloadClient[] = [
		{
			id: 'dc-1',
			name: 'Seed Client',
			client_type: 'qbittorrent',
			base_url: 'http://localhost:8080',
			username: null,
			category: null,
			enabled: true,
			has_password: false
		}
	];

	await routeJson(page, '**/api/v1/settings/download-clients**', async (route) => {
		const req = route.request();
		const method = req.method();
		const url = new URL(req.url());
		const path = url.pathname;

		if (path.endsWith('/settings/download-clients') && method === 'GET') {
			await route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({ items, total: items.length, limit: 100, offset: 0 })
			});
			return;
		}

		if (path.endsWith('/settings/download-clients') && method === 'POST') {
			const body = req.postDataJSON() as Partial<DownloadClient>;
			const created: DownloadClient = {
				id: `dc-${nextId++}`,
				name: body.name ?? 'Unnamed',
				client_type: body.client_type ?? 'qbittorrent',
				base_url: body.base_url ?? 'http://localhost:8080',
				username: body.username ?? null,
				category: body.category ?? null,
				enabled: body.enabled ?? true,
				has_password: false
			};
			items.push(created);
			await route.fulfill({ status: 201, contentType: 'application/json', body: JSON.stringify(created) });
			return;
		}

		const id = path.split('/').pop() ?? '';
		const idx = items.findIndex((item) => item.id === id);

		if (idx === -1) {
			await route.fulfill({ status: 404, contentType: 'application/json', body: JSON.stringify({ error: 'not found' }) });
			return;
		}

		if (method === 'PUT') {
			const patch = req.postDataJSON() as Partial<DownloadClient>;
			items[idx] = { ...items[idx], ...patch };
			await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(items[idx]) });
			return;
		}

		if (method === 'DELETE') {
			items.splice(idx, 1);
			await route.fulfill({ status: 204, contentType: 'application/json', body: '{}' });
			return;
		}

		await route.fallback();
	});
}

export async function mockIndexersCrud(page: Page): Promise<void> {
	let nextId = 2;
	const items: Indexer[] = [
		{
			id: 'idx-1',
			name: 'Seed Indexer',
			base_url: 'http://localhost:9117',
			protocol: 'torznab',
			enabled: true,
			has_api_key: false
		}
	];

	await routeJson(page, '**/api/v1/indexers/test', async (route) => {
		if (route.request().method() !== 'POST') {
			await route.fallback();
			return;
		}

		await route.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify({
				success: true,
				message: 'ok',
				protocol: 'torznab',
				capabilities: {
					supports_search: true,
					supports_rss: true,
					supports_capabilities_detection: true,
					supports_categories: true,
					supported_categories: ['music']
				}
			})
		});
	});

	await routeJson(page, '**/api/v1/settings/indexers**', async (route) => {
		const req = route.request();
		const method = req.method();
		const url = new URL(req.url());
		const path = url.pathname;

		if (path.endsWith('/settings/indexers') && method === 'GET') {
			await route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({ items, total: items.length, limit: 100, offset: 0 })
			});
			return;
		}

		if (path.endsWith('/settings/indexers') && method === 'POST') {
			const body = req.postDataJSON() as Partial<Indexer>;
			const created: Indexer = {
				id: `idx-${nextId++}`,
				name: body.name ?? 'Unnamed',
				base_url: body.base_url ?? 'http://localhost:9117',
				protocol: body.protocol ?? 'torznab',
				enabled: body.enabled ?? true,
				has_api_key: Boolean(body.has_api_key)
			};
			items.push(created);
			await route.fulfill({ status: 201, contentType: 'application/json', body: JSON.stringify(created) });
			return;
		}

		const id = path.split('/').pop() ?? '';
		const idx = items.findIndex((item) => item.id === id);
		if (idx === -1) {
			await route.fulfill({ status: 404, contentType: 'application/json', body: JSON.stringify({ error: 'not found' }) });
			return;
		}

		if (method === 'PUT') {
			const patch = req.postDataJSON() as Partial<Indexer>;
			items[idx] = { ...items[idx], ...patch };
			await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(items[idx]) });
			return;
		}

		if (method === 'DELETE') {
			items.splice(idx, 1);
			await route.fulfill({ status: 204, contentType: 'application/json', body: '{}' });
			return;
		}

		await route.fallback();
	});
}

export async function mockQualityProfilesCrud(page: Page): Promise<void> {
	let nextId = 2;
	const items: QualityProfile[] = [
		{
			id: 'qp-1',
			name: 'Seed Quality',
			allowed_qualities: ['FLAC'],
			upgrade_allowed: true,
			cutoff_quality: 'FLAC'
		}
	];

	await routeJson(page, '**/api/v1/settings/quality-profiles**', async (route) => {
		const req = route.request();
		const method = req.method();
		const path = new URL(req.url()).pathname;

		if (path.endsWith('/settings/quality-profiles') && method === 'GET') {
			await route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({ items, total: items.length, limit: 100, offset: 0 })
			});
			return;
		}

		if (path.endsWith('/settings/quality-profiles') && method === 'POST') {
			const body = req.postDataJSON() as Partial<QualityProfile>;
			const created: QualityProfile = {
				id: `qp-${nextId++}`,
				name: body.name ?? 'Unnamed',
				allowed_qualities: body.allowed_qualities ?? ['FLAC'],
				upgrade_allowed: body.upgrade_allowed ?? true,
				cutoff_quality: body.cutoff_quality ?? null
			};
			items.push(created);
			await route.fulfill({ status: 201, contentType: 'application/json', body: JSON.stringify(created) });
			return;
		}

		const id = path.split('/').pop() ?? '';
		const idx = items.findIndex((item) => item.id === id);
		if (idx === -1) {
			await route.fulfill({ status: 404, contentType: 'application/json', body: JSON.stringify({ error: 'not found' }) });
			return;
		}

		if (method === 'PUT') {
			const patch = req.postDataJSON() as Partial<QualityProfile>;
			items[idx] = { ...items[idx], ...patch };
			await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(items[idx]) });
			return;
		}

		if (method === 'DELETE') {
			items.splice(idx, 1);
			await route.fulfill({ status: 204, contentType: 'application/json', body: '{}' });
			return;
		}

		await route.fallback();
	});
}

export async function mockMetadataProfilesCrud(page: Page): Promise<void> {
	let nextId = 2;
	const items: MetadataProfile[] = [
		{
			id: 'mp-1',
			name: 'Seed Metadata',
			primary_album_types: ['Album'],
			secondary_album_types: [],
			release_statuses: ['Official']
		}
	];

	await routeJson(page, '**/api/v1/settings/metadata-profiles**', async (route) => {
		const req = route.request();
		const method = req.method();
		const path = new URL(req.url()).pathname;

		if (path.endsWith('/settings/metadata-profiles') && method === 'GET') {
			await route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({ items, total: items.length, limit: 100, offset: 0 })
			});
			return;
		}

		if (path.endsWith('/settings/metadata-profiles') && method === 'POST') {
			const body = req.postDataJSON() as Partial<MetadataProfile>;
			const created: MetadataProfile = {
				id: `mp-${nextId++}`,
				name: body.name ?? 'Unnamed',
				primary_album_types: body.primary_album_types ?? ['Album'],
				secondary_album_types: body.secondary_album_types ?? [],
				release_statuses: body.release_statuses ?? ['Official']
			};
			items.push(created);
			await route.fulfill({ status: 201, contentType: 'application/json', body: JSON.stringify(created) });
			return;
		}

		const id = path.split('/').pop() ?? '';
		const idx = items.findIndex((item) => item.id === id);
		if (idx === -1) {
			await route.fulfill({ status: 404, contentType: 'application/json', body: JSON.stringify({ error: 'not found' }) });
			return;
		}

		if (method === 'PUT') {
			const patch = req.postDataJSON() as Partial<MetadataProfile>;
			items[idx] = { ...items[idx], ...patch };
			await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(items[idx]) });
			return;
		}

		if (method === 'DELETE') {
			items.splice(idx, 1);
			await route.fulfill({ status: 204, contentType: 'application/json', body: '{}' });
			return;
		}

		await route.fallback();
	});
}

export async function mockAllSettingsCrud(page: Page): Promise<void> {
	await Promise.all([
		mockDownloadClientsCrud(page),
		mockIndexersCrud(page),
		mockQualityProfilesCrud(page),
		mockMetadataProfilesCrud(page)
	]);
}
