import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ApiError, sseUrl } from '$lib/api';

describe('ApiError', () => {
	it('is an instance of Error', () => {
		const err = new ApiError('something failed', 404);
		expect(err).toBeInstanceOf(Error);
		expect(err).toBeInstanceOf(ApiError);
	});

	it('stores status and message', () => {
		const err = new ApiError('not found', 404);
		expect(err.message).toBe('not found');
		expect(err.status).toBe(404);
	});

	it('stores optional body', () => {
		const body = { error: 'resource not found' };
		const err = new ApiError('not found', 404, body);
		expect(err.body).toEqual(body);
	});

	it('body is undefined when not provided', () => {
		const err = new ApiError('server error', 500);
		expect(err.body).toBeUndefined();
	});
});

describe('sseUrl', () => {
	beforeEach(() => {
		// Reset the module's API_BASE by clearing env override
		vi.unstubAllEnvs();
	});

	it('appends path to default API base', () => {
		const url = sseUrl('/api/v1/events');
		expect(url).toBe('http://127.0.0.1:5150/api/v1/events');
	});

	it('appends a nested SSE path', () => {
		const url = sseUrl('/api/v1/events/job-status');
		expect(url).toBe('http://127.0.0.1:5150/api/v1/events/job-status');
	});
});
