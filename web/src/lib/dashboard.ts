/**
 * Pure helper functions extracted from the dashboard component.
 * Keeping them in a separate module makes them unit-testable without
 * mounting the full Svelte component.
 */

export type StreamConnectionState = 'connecting' | 'connected' | 'reconnecting' | 'disconnected';
export type StreamKey = 'events' | 'queue' | 'processing' | 'tasks';

export const ALL_STREAM_KEYS: StreamKey[] = ['events', 'queue', 'processing', 'tasks'];

/** Collapse per-stream states into a single aggregate status for the UI pill. */
export function aggregateStreamState(
	states: Record<StreamKey, StreamConnectionState>
): 'connected' | 'disconnected' | 'reconnecting' | 'connecting' {
	const values = ALL_STREAM_KEYS.map((k) => states[k]);
	if (values.every((s) => s === 'connected')) return 'connected';
	if (values.every((s) => s === 'disconnected')) return 'disconnected';
	if (values.some((s) => s === 'reconnecting')) return 'reconnecting';
	if (values.some((s) => s === 'connecting')) return 'connecting';
	return 'reconnecting';
}

/** Exponential back-off delay in milliseconds for reconnect attempts. */
export function backoffMs(
	attempts: number,
	baseMs = 1_000,
	maxMs = 30_000
): number {
	return Math.min(baseMs * 2 ** attempts, maxMs);
}

const STALE_THRESHOLD_MS = 60_000;

/** Returns true when the data timestamp is older than the stale threshold. */
export function isStale(date: Date | null, now: number, thresholdMs = STALE_THRESHOLD_MS): boolean {
	if (!date) return true;
	return now - date.getTime() > thresholdMs;
}

/** Human-readable age string relative to `now` in milliseconds. */
export function formatAge(date: Date | null, now: number): string {
	if (!date) return 'never';
	const secs = Math.floor((now - date.getTime()) / 1000);
	if (secs < 5) return 'just now';
	if (secs < 60) return `${secs}s ago`;
	const mins = Math.floor(secs / 60);
	if (mins < 60) return `${mins}m ago`;
	return `${Math.floor(mins / 60)}h ago`;
}
