import { ApiError } from '$lib/api';
import type { ApiValidationError } from '$lib/types';

export interface FieldValidationRule {
	field: string;
	messages: string[];
}

export interface ClassifiedFormError {
	fieldErrors: Record<string, string>;
	bannerMessage: string;
}

function getBodyMessage(body: unknown): string {
	if (typeof body === 'object' && body !== null && 'error' in body) {
		return String((body as { error?: unknown }).error ?? '');
	}
	return '';
}

function extractStructuredFieldErrors(body: unknown): Record<string, string> {
	const fieldErrors: Record<string, string> = {};
	if (
		typeof body === 'object' &&
		body !== null &&
		'fields' in body &&
		Array.isArray((body as ApiValidationError).fields)
	) {
		for (const fieldError of (body as ApiValidationError).fields ?? []) {
			if (fieldError.field && fieldError.message) {
				fieldErrors[fieldError.field] = fieldError.message;
			}
		}
	}
	return fieldErrors;
}

export function classifyFormError(error: unknown, rules: FieldValidationRule[]): ClassifiedFormError {
	const fieldErrors: Record<string, string> = {};

	if (error instanceof ApiError) {
		Object.assign(fieldErrors, extractStructuredFieldErrors(error.body));

		const message = error.message.trim();
		const bodyMessage = getBodyMessage(error.body).trim();
		const combined = [message, bodyMessage].filter(Boolean).join(' ');

		for (const rule of rules) {
			if (fieldErrors[rule.field]) continue;
			if (rule.messages.some((candidate) => combined.includes(candidate))) {
				fieldErrors[rule.field] = message || bodyMessage || 'Invalid value';
			}
		}

		return {
			fieldErrors,
			bannerMessage: Object.keys(fieldErrors).length > 0 ? '' : message || bodyMessage
		};
	}

	return {
		fieldErrors,
		bannerMessage: error instanceof Error ? error.message : 'Save failed.'
	};
}