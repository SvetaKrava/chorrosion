import { describe, it, expect } from 'vitest';
import {
	ALL_STREAM_KEYS,
	aggregateStreamState,
	backoffMs,
	formatAge,
	isStale,
	needsReconnect,
	scheduleSummary,
	STREAM_LABELS,
	stateColor,
	streamHealthClass
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

describe('streamHealthClass', () => {
	it('returns the state string unchanged for each state', () => {
		const states: StreamConnectionState[] = ['connected', 'connecting', 'reconnecting', 'disconnected'];
		for (const s of states) {
			expect(streamHealthClass(s)).toBe(s);
		}
	});
});

describe('needsReconnect', () => {
	it('returns false when all streams are connected', () => {
		expect(needsReconnect(allStates('connected'))).toBe(false);
	});

	it('returns false when all streams are connecting', () => {
		expect(needsReconnect(allStates('connecting'))).toBe(false);
	});

	it('returns true when any stream is reconnecting', () => {
		const states = allStates('connected');
		states.queue = 'reconnecting';
		expect(needsReconnect(states)).toBe(true);
	});

	it('returns true when any stream is disconnected', () => {
		const states = allStates('connected');
		states.events = 'disconnected';
		expect(needsReconnect(states)).toBe(true);
	});

	it('returns true when all streams are disconnected', () => {
		expect(needsReconnect(allStates('disconnected'))).toBe(true);
	});
});

describe('STREAM_LABELS', () => {
	it('has a label for every stream key', () => {
		for (const key of ALL_STREAM_KEYS) {
			expect(typeof STREAM_LABELS[key]).toBe('string');
			expect(STREAM_LABELS[key].length).toBeGreaterThan(0);
		}
	});
});

describe('stateColor', () => {
	it('maps known states to expected CSS classes', () => {
		expect(stateColor('downloading')).toBe('state-active');
		expect(stateColor('queued')).toBe('state-queued');
		expect(stateColor('paused')).toBe('state-paused');
		expect(stateColor('completed')).toBe('state-done');
		expect(stateColor('error')).toBe('state-error');
	});

	it('returns state-unknown for unrecognised states', () => {
		expect(stateColor('pending')).toBe('state-unknown');
		expect(stateColor('')).toBe('state-unknown');
		expect(stateColor('DOWNLOADING')).toBe('state-unknown');
	});
});

describe('scheduleSummary', () => {
	it('renders seconds for intervals under 60s', () => {
		expect(scheduleSummary(1)).toBe('Every 1s');
		expect(scheduleSummary(30)).toBe('Every 30s');
		expect(scheduleSummary(59)).toBe('Every 59s');
	});

	it('renders minutes for intervals between 60s and 3600s', () => {
		expect(scheduleSummary(60)).toBe('Every 1m');
		expect(scheduleSummary(90)).toBe('Every 2m');
		expect(scheduleSummary(3599)).toBe('Every 60m');
	});

	it('renders hours for intervals >= 3600s', () => {
		expect(scheduleSummary(3600)).toBe('Every 1h');
		expect(scheduleSummary(7200)).toBe('Every 2h');
		expect(scheduleSummary(5400)).toBe('Every 2h');
	});
});
