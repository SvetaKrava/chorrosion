import { describe, it, expect } from 'vitest';
import {
	ALL_STREAM_KEYS,
	aggregateStreamState,
	backoffMs,
	formatAge,
	isStale
} from '$lib/dashboard';
import type { StreamConnectionState, StreamKey } from '$lib/dashboard';

// Helper to build a uniform state record.
function allStates(state: StreamConnectionState): Record<StreamKey, StreamConnectionState> {
	return Object.fromEntries(ALL_STREAM_KEYS.map((k) => [k, state])) as Record<
		StreamKey,
		StreamConnectionState
	>;
}

describe('aggregateStreamState', () => {
	it('returns connected when every stream is connected', () => {
		expect(aggregateStreamState(allStates('connected'))).toBe('connected');
	});

	it('returns disconnected when every stream is disconnected', () => {
		expect(aggregateStreamState(allStates('disconnected'))).toBe('disconnected');
	});

	// Regression: mixed states must NOT collapse to reconnecting unless at least
	// one stream is actually reconnecting (the bug that caused the false RECONNECTING pill).
	it('returns connecting — not reconnecting — when streams are connecting', () => {
		const states = allStates('connected');
		states.processing = 'connecting';
		states.tasks = 'connecting';
		expect(aggregateStreamState(states)).toBe('connecting');
	});

	it('returns reconnecting when any stream is reconnecting', () => {
		const states = allStates('connected');
		states.queue = 'reconnecting';
		expect(aggregateStreamState(states)).toBe('reconnecting');
	});

	it('reconnecting takes priority over connecting', () => {
		const states = allStates('connected');
		states.processing = 'reconnecting';
		states.tasks = 'connecting';
		expect(aggregateStreamState(states)).toBe('reconnecting');
	});

	it('returns connecting when some streams connected, some connecting', () => {
		const states: Record<StreamKey, StreamConnectionState> = {
			events: 'connected',
			queue: 'connected',
			processing: 'connecting',
			tasks: 'connecting'
		};
		expect(aggregateStreamState(states)).toBe('connecting');
	});

	it('returns connecting when all streams are connecting', () => {
		expect(aggregateStreamState(allStates('connecting'))).toBe('connecting');
	});
});

describe('backoffMs', () => {
	it('returns base on first attempt (0)', () => {
		expect(backoffMs(0)).toBe(1_000);
	});

	it('doubles with each attempt', () => {
		expect(backoffMs(1)).toBe(2_000);
		expect(backoffMs(2)).toBe(4_000);
		expect(backoffMs(3)).toBe(8_000);
	});

	it('caps at max', () => {
		expect(backoffMs(100)).toBe(30_000);
	});

	it('accepts custom base and max', () => {
		expect(backoffMs(0, 500, 5_000)).toBe(500);
		expect(backoffMs(10, 500, 5_000)).toBe(5_000);
	});
});

describe('isStale', () => {
	const now = Date.now();

	it('returns true for null', () => {
		expect(isStale(null, now)).toBe(true);
	});

	it('returns false for a recent date', () => {
		expect(isStale(new Date(now - 5_000), now)).toBe(false);
	});

	it('returns true when older than threshold', () => {
		expect(isStale(new Date(now - 61_000), now)).toBe(true);
	});

	it('accepts a custom threshold', () => {
		expect(isStale(new Date(now - 10_000), now, 5_000)).toBe(true);
		expect(isStale(new Date(now - 4_000), now, 5_000)).toBe(false);
	});
});

describe('formatAge', () => {
	const now = Date.now();

	it('returns "never" for null', () => {
		expect(formatAge(null, now)).toBe('never');
	});

	it('returns "just now" for very recent dates', () => {
		expect(formatAge(new Date(now - 2_000), now)).toBe('just now');
	});

	it('returns seconds label', () => {
		expect(formatAge(new Date(now - 10_000), now)).toBe('10s ago');
	});

	it('returns minutes label', () => {
		expect(formatAge(new Date(now - 120_000), now)).toBe('2m ago');
	});

	it('returns hours label', () => {
		expect(formatAge(new Date(now - 3_600_000 * 2), now)).toBe('2h ago');
	});
});
