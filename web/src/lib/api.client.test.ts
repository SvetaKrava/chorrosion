import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	ApiError,
	createDownloadClient,
	createQualityProfile,
	deleteMetadataProfile,
	getDownloadClients,
	getIndexers,
	getMetadataProfiles,
	getQualityProfiles,
	parseApiError,
	updateIndexer
} from '$lib/api';

describe('parseApiError', () => {
	it('returns the API error field when present', () => {
		expect(parseApiError({ error: 'bad request' }, 'fallback')).toBe('bad request');
	});

	it('falls back when body does not include error', () => {
		expect(parseApiError({ message: 'nope' }, 'fallback')).toBe('fallback');
	});

	it('falls back for non-object values', () => {
		expect(parseApiError('oops', 'fallback')).toBe('fallback');
	});
});

describe('settings API client methods', () => {
	const fetchMock = vi.fn<typeof fetch>();

	beforeEach(() => {
		vi.stubGlobal('fetch', fetchMock);
	});

	afterEach(() => {
		vi.unstubAllGlobals();
		fetchMock.mockReset();
	});

	function mockJson(status: number, body: unknown): Response {
		return new Response(JSON.stringify(body), {
			status,
			headers: { 'Content-Type': 'application/json' }
		});
	}

	it('gets download clients with query params', async () => {
		fetchMock.mockResolvedValueOnce(
			mockJson(200, { items: [], total: 0, limit: 50, offset: 0 })
		);

		await getDownloadClients({ limit: 50, offset: 0 });

		expect(fetchMock).toHaveBeenCalledOnce();
		expect(String(fetchMock.mock.calls[0][0])).toContain(
			'/api/v1/settings/download-clients?limit=50&offset=0'
		);
	});

	it('gets indexers list', async () => {
		fetchMock.mockResolvedValueOnce(
			mockJson(200, { items: [], total: 0, limit: 25, offset: 0 })
		);

		await getIndexers({ limit: 25 });

		expect(String(fetchMock.mock.calls[0][0])).toContain('/api/v1/settings/indexers?limit=25');
	});

	it('gets quality profiles list', async () => {
		fetchMock.mockResolvedValueOnce(
			mockJson(200, { items: [], total: 0, limit: 100, offset: 0 })
		);

		await getQualityProfiles({ limit: 100 });

		expect(String(fetchMock.mock.calls[0][0])).toContain('/api/v1/settings/quality-profiles?limit=100');
	});

	it('gets metadata profiles list', async () => {
		fetchMock.mockResolvedValueOnce(
			mockJson(200, { items: [], total: 0, limit: 100, offset: 0 })
		);

		await getMetadataProfiles({ limit: 100 });

		expect(String(fetchMock.mock.calls[0][0])).toContain('/api/v1/settings/metadata-profiles?limit=100');
	});

	it('creates a download client', async () => {
		fetchMock.mockResolvedValueOnce(
			mockJson(201, {
				id: 'dc-1',
				name: 'Main',
				client_type: 'qbittorrent',
				base_url: 'http://localhost:8080',
				username: null,
				category: null,
				enabled: true,
				has_password: false
			})
		);

		await createDownloadClient({
			name: 'Main',
			client_type: 'qbittorrent',
			base_url: 'http://localhost:8080',
			enabled: true
		});

		const [, init] = fetchMock.mock.calls[0];
		expect(init?.method).toBe('POST');
		expect(String(init?.body)).toContain('qbittorrent');
	});

	it('updates an indexer', async () => {
		fetchMock.mockResolvedValueOnce(
			mockJson(200, {
				id: 'idx-1',
				name: 'Idx',
				base_url: 'http://localhost:9117',
				protocol: 'torznab',
				enabled: false,
				has_api_key: false
			})
		);

		await updateIndexer('idx-1', { enabled: false });

		const [url, init] = fetchMock.mock.calls[0];
		expect(String(url)).toContain('/api/v1/settings/indexers/idx-1');
		expect(init?.method).toBe('PUT');
	});

	it('creates a quality profile', async () => {
		fetchMock.mockResolvedValueOnce(
			mockJson(201, {
				id: 'qp-1',
				name: 'Lossless',
				allowed_qualities: ['FLAC'],
				upgrade_allowed: true,
				cutoff_quality: 'FLAC'
			})
		);

		await createQualityProfile({
			name: 'Lossless',
			allowed_qualities: ['FLAC'],
			upgrade_allowed: true,
			cutoff_quality: 'FLAC'
		});

		const [, init] = fetchMock.mock.calls[0];
		expect(init?.method).toBe('POST');
	});

	it('deletes a metadata profile', async () => {
		fetchMock.mockResolvedValueOnce(mockJson(204, {}));

		await deleteMetadataProfile('mp-1');

		const [url, init] = fetchMock.mock.calls[0];
		expect(String(url)).toContain('/api/v1/settings/metadata-profiles/mp-1');
		expect(init?.method).toBe('DELETE');
	});

	it('throws ApiError with parsed message for 422/validation style body', async () => {
		fetchMock.mockResolvedValueOnce(
			mockJson(422, {
				error: 'validation failed',
				fields: [{ field: 'name', message: 'required' }]
			})
		);

		const promise = getDownloadClients();
		await expect(promise).rejects.toBeInstanceOf(ApiError);
		await expect(promise).rejects.toMatchObject({ message: 'validation failed', status: 422 });
	});

	it('throws fallback ApiError message for 500 with non-json body', async () => {
		fetchMock.mockResolvedValueOnce(new Response('server exploded', { status: 500 }));

		await expect(getIndexers()).rejects.toMatchObject({
			message: 'Request failed with status 500',
			status: 500
		});
	});
});
