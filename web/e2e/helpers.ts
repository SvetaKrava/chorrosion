import type { Page } from '@playwright/test';

/** The mock API base — matches the default VITE_CHORROSION_API_BASE. */
const API_BASE = 'http://127.0.0.1:5150';

export const MOCK_USER = {
	isAuthenticated: true,
	username: 'testuser',
	token: 'mock-e2e-token'
};

/**
 * Inject auth state into sessionStorage before the page loads so tests can
 * skip the login form and start on an authenticated page.
 * Must be called before `page.goto()`.
 */
export async function injectAuth(page: Page): Promise<void> {
	await page.addInitScript((auth) => {
		sessionStorage.setItem('auth_state', JSON.stringify(auth));
	}, MOCK_USER);
}

/**
 * Inject a mock EventSource into the page that immediately fires `onopen`
 * and a `connected` event, then stays open forever (no onerror).
 * This simulates a healthy SSE connection without needing a real server.
 * Must be called before `page.goto()`.
 */
export async function injectConnectedEventSource(page: Page): Promise<void> {
	await page.addInitScript(() => {
		class MockEventSource extends EventTarget {
			static CONNECTING = 0;
			static OPEN = 1;
			static CLOSED = 2;
			readyState = MockEventSource.CONNECTING;
			onopen: ((ev: Event) => void) | null = null;
			onerror: ((ev: Event) => void) | null = null;

			constructor(_url: string, _opts?: EventSourceInit) {
				super();
				setTimeout(() => {
					if (this.readyState === MockEventSource.CLOSED) return;
					this.readyState = MockEventSource.OPEN;
					const openEvent = new Event('open');
					if (this.onopen) this.onopen(openEvent);
					this.dispatchEvent(new MessageEvent('connected', { data: '{}' }));
				}, 30);
			}

			close() {
				this.readyState = MockEventSource.CLOSED;
			}
		}
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		(window as any).EventSource = MockEventSource;
	});
}

/**
 * Inject a mock EventSource that immediately fires `onerror`, simulating a
 * backend that is unreachable. Must be called before `page.goto()`.
 */
export async function injectFailingEventSource(page: Page): Promise<void> {
	await page.addInitScript(() => {
		class FailingEventSource extends EventTarget {
			static CONNECTING = 0;
			static OPEN = 1;
			static CLOSED = 2;
			readyState = FailingEventSource.CONNECTING;
			onopen: ((ev: Event) => void) | null = null;
			onerror: ((ev: Event) => void) | null = null;

			constructor(_url: string, _opts?: EventSourceInit) {
				super();
				setTimeout(() => {
					if (this.readyState === FailingEventSource.CLOSED) return;
					const errEvent = new Event('error');
					if (this.onerror) this.onerror(errEvent);
				}, 30);
			}

			close() {
				this.readyState = FailingEventSource.CLOSED;
			}
		}
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		(window as any).EventSource = FailingEventSource;
	});
}

export async function mockApiRoutes(page: Page): Promise<void> {
	await page.route(`${API_BASE}/api/v1/auth/forms/login`, (route) =>
		route.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify({
				authenticated: true,
				username: 'testuser',
				permission_level: 'admin'
			})
		})
	);

	await page.route(`${API_BASE}/api/v1/auth/forms/logout`, (route) =>
		route.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify({ logged_out: true })
		})
	);

	// Appearance settings
	await page.route(`${API_BASE}/api/v1/settings/appearance`, (route) =>
		route.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify({
				theme_mode: 'system',
				mobile_breakpoint_px: 768,
				mobile_compact_layout: false,
				touch_targets_optimized: false,
				keyboard_shortcuts_enabled: true,
				shortcut_profile: 'standard',
				bulk_operations_enabled: true,
				bulk_selection_limit: 250,
				bulk_action_confirmation: true,
				advanced_filtering_enabled: false,
				default_filter_operator: 'and',
				max_filter_clauses: 10,
				filter_history_enabled: true,
				filter_history_limit: 20
			})
		})
	);

	// Activity snapshots
	const emptyList = { items: [], total: 0 };
	await page.route(`${API_BASE}/api/v1/activity/queue`, (route) =>
		route.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify(emptyList)
		})
	);
	await page.route(`${API_BASE}/api/v1/activity/processing`, (route) =>
		route.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify(emptyList)
		})
	);
	await page.route(`${API_BASE}/api/v1/system/tasks`, (route) =>
		route.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify({ items: [], total: 0, max_concurrent_jobs: 4 })
		})
	);

	// Catalog endpoints
	const emptyPage = { items: [], total: 0, limit: 25, offset: 0 };
	await page.route(`${API_BASE}/api/v1/artists**`, (route) =>
		route.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify(emptyPage)
		})
	);
	await page.route(`${API_BASE}/api/v1/albums**`, (route) =>
		route.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify(emptyPage)
		})
	);
}
