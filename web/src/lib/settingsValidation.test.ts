import { describe, expect, it } from 'vitest';
import { ApiError } from '$lib/api';
import { classifyFormError, mapClassifiedSaveErrorToUiState } from '$lib/settingsValidation';

describe('classifyFormError', () => {
	it('returns structured field errors from API response bodies', () => {
		const error = new ApiError('Validation failed', 400, {
			fields: [{ field: 'name', message: 'Name is required.' }]
		});

		const result = classifyFormError(error, []);

		expect(result.fieldErrors).toEqual({ name: 'Name is required.' });
		expect(result.bannerMessage).toBe('');
	});

	it('returns banner error when no field errors are found', () => {
		const error = new ApiError('', 409, { error: 'Download client already exists' });

		const result = classifyFormError(error, []);

		expect(result.fieldErrors).toEqual({});
		expect(result.bannerMessage).toBe('Download client already exists');
	});

	it('matches field errors using message rules', () => {
		const error = new ApiError('Indexer name already exists', 409, {});

		const result = classifyFormError(error, [
			{ field: 'name', messages: ['already exists'] }
		]);

		expect(result.fieldErrors).toEqual({ name: 'Indexer name already exists' });
		expect(result.bannerMessage).toBe('');
	});

	it('handles non-ApiError values as banner-only failures', () => {
		const result = classifyFormError(new Error('Network failed'), [
			{ field: 'name', messages: ['already exists'] }
		]);

		expect(result.fieldErrors).toEqual({});
		expect(result.bannerMessage).toBe('Network failed');
	});
});

describe('mapClassifiedSaveErrorToUiState', () => {
	it('merges field errors and keeps banner clear when field mapping succeeds', () => {
		const result = mapClassifiedSaveErrorToUiState(
			{
				fieldErrors: { name: 'Name already exists.' },
				bannerMessage: ''
			},
			{ base_url: 'Base URL is required.' }
		);

		expect(result.formErrors).toEqual({
			base_url: 'Base URL is required.',
			name: 'Name already exists.'
		});
		expect(result.saveStatus).toBe('idle');
		expect(result.saveError).toBe('');
	});

	it('returns banner error state when no field errors are present', () => {
		const result = mapClassifiedSaveErrorToUiState(
			{
				fieldErrors: {},
				bannerMessage: 'Request failed.'
			},
			{ base_url: 'Existing field error' }
		);

		expect(result.formErrors).toEqual({ base_url: 'Existing field error' });
		expect(result.saveStatus).toBe('error');
		expect(result.saveError).toBe('Request failed.');
	});

	it('uses fallback message when banner message is empty', () => {
		const result = mapClassifiedSaveErrorToUiState(
			{
				fieldErrors: {},
				bannerMessage: ''
			},
			{}
		);

		expect(result.formErrors).toEqual({});
		expect(result.saveStatus).toBe('error');
		expect(result.saveError).toBe('Save failed.');
	});
});
