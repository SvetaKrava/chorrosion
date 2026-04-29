<script lang="ts">
	import {
		ApiError,
		getAppearanceSettings,
		getProcessingSnapshot,
		getQueueSnapshot,
		getTasksSnapshot,
		login,
		logout,
		sseUrl,
		updateAppearanceSettings
	} from '$lib/api';
	import type { AppearanceSettings, GenericListResponse } from '$lib/types';

	const DEFAULT_ERROR = 'Request failed. Please verify API server settings and try again.';

	let loggedIn = $state(false);
	let username = $state('');
	let password = $state('');
	let loginError = $state('');
	let busy = $state(false);

	let settings = $state<AppearanceSettings | null>(null);
	let settingsError = $state('');
	let settingsSaved = $state('');

	let queue = $state<GenericListResponse | null>(null);
	let processing = $state<GenericListResponse | null>(null);
	let tasks = $state<GenericListResponse | null>(null);

	let pulse = $state('idle');
	let pulseTick = $state(0);
	let streamState = $state('disconnected');

	let eventStreams: EventSource[] = [];

	const safeParse = (input: string): unknown => {
		try {
			return JSON.parse(input);
		} catch {
			return null;
		}
	};

	function apiErrorMessage(error: unknown): string {
		if (error instanceof ApiError) {
			return error.message;
		}
		if (error instanceof Error) {
			return error.message;
		}
		return DEFAULT_ERROR;
	}

	async function hydrateData(): Promise<void> {
		settingsError = '';
		settingsSaved = '';
		try {
			const [appearance, queueSnapshot, processingSnapshot, tasksSnapshot] = await Promise.all([
				getAppearanceSettings(),
				getQueueSnapshot(),
				getProcessingSnapshot(),
				getTasksSnapshot()
			]);
			settings = appearance;
			queue = queueSnapshot;
			processing = processingSnapshot;
			tasks = tasksSnapshot;
		} catch (error) {
			settingsError = apiErrorMessage(error);
		}
	}

	function attachSse(): void {
		detachSse();
		streamState = 'connecting';

		const pulseStream = new EventSource(sseUrl('/api/v1/events'), { withCredentials: true });
		pulseStream.onmessage = (event) => {
			const payload = safeParse(event.data) as { status?: string; tick?: number } | null;
			pulse = payload?.status ?? 'active';
			pulseTick = payload?.tick ?? pulseTick;
			streamState = 'connected';
		};

		const queueStream = new EventSource(sseUrl('/api/v1/events/download-progress'), {
			withCredentials: true
		});
		queueStream.onmessage = (event) => {
			const payload = safeParse(event.data) as { queue?: GenericListResponse } | null;
			if (payload?.queue) queue = payload.queue;
		};

		const processingStream = new EventSource(sseUrl('/api/v1/events/import-progress'), {
			withCredentials: true
		});
		processingStream.onmessage = (event) => {
			const payload = safeParse(event.data) as { processing?: GenericListResponse } | null;
			if (payload?.processing) processing = payload.processing;
		};

		const taskStream = new EventSource(sseUrl('/api/v1/events/job-status'), {
			withCredentials: true
		});
		taskStream.onmessage = (event) => {
			const payload = safeParse(event.data) as { tasks?: GenericListResponse } | null;
			if (payload?.tasks) tasks = payload.tasks;
		};

		for (const stream of [pulseStream, queueStream, processingStream, taskStream]) {
			stream.onerror = () => {
				if (loggedIn) {
					streamState = 'reconnecting';
				}
			};
		}

		eventStreams = [pulseStream, queueStream, processingStream, taskStream];
	}

	function detachSse(): void {
		for (const stream of eventStreams) {
			stream.close();
		}
		eventStreams = [];
		streamState = 'disconnected';
	}

	async function onLoginSubmit(event: SubmitEvent): Promise<void> {
		event.preventDefault();
		loginError = '';
		busy = true;
		try {
			await login(username.trim(), password);
			loggedIn = true;
			password = '';
			await hydrateData();
			attachSse();
		} catch (error) {
			loginError = apiErrorMessage(error);
		} finally {
			busy = false;
		}
	}

	async function onLogout(): Promise<void> {
		busy = true;
		try {
			await logout();
		} catch {
			// Best-effort logout; local state must still reset.
		}
		loggedIn = false;
		settings = null;
		queue = null;
		processing = null;
		tasks = null;
		detachSse();
		busy = false;
	}

	async function saveSettings(): Promise<void> {
		if (!settings) return;
		settingsSaved = '';
		settingsError = '';
		busy = true;
		try {
			settings = await updateAppearanceSettings(settings);
			settingsSaved = 'Appearance settings saved.';
		} catch (error) {
			settingsError = apiErrorMessage(error);
		} finally {
			busy = false;
		}
	}

	$effect(() => {
		return () => detachSse();
	});
</script>

<main class="page">
	<section class="hero">
		<p class="eyebrow">Phase 11 UI Foundation</p>
		<h1>Chorrosion Control Deck</h1>
		<p>
			Editorial, realtime-first control surface for auth, activity, and appearance preferences.
		</p>
	</section>

	{#if !loggedIn}
		<section class="panel">
			<h2>Sign In</h2>
			<form class="login-form" onsubmit={onLoginSubmit}>
				<label>
					<span>Username</span>
					<input bind:value={username} type="text" autocomplete="username" required />
				</label>
				<label>
					<span>Password</span>
					<input bind:value={password} type="password" autocomplete="current-password" required />
				</label>
				<button class="primary" type="submit" disabled={busy}>
					{busy ? 'Signing In…' : 'Sign In'}
				</button>
			</form>
			{#if loginError}
				<p class="error">{loginError}</p>
			{/if}
		</section>
	{:else}
		<section class="topbar">
			<p>
				Session: <strong>{username || 'authenticated'}</strong>
			</p>
			<p>
				Stream: <span class="pill">{streamState}</span> · Pulse: {pulse} #{pulseTick}
			</p>
			<button class="ghost" type="button" onclick={onLogout} disabled={busy}>Log Out</button>
		</section>

		<section class="dashboard-grid">
			<article class="panel stat-panel">
				<h3>Download Queue</h3>
				<p class="stat">{queue?.total ?? 0}</p>
				<p class="meta">Live from /events/download-progress</p>
			</article>
			<article class="panel stat-panel">
				<h3>Import Processing</h3>
				<p class="stat">{processing?.total ?? 0}</p>
				<p class="meta">Live from /events/import-progress</p>
			</article>
			<article class="panel stat-panel">
				<h3>Scheduled Tasks</h3>
				<p class="stat">{tasks?.total ?? 0}</p>
				<p class="meta">Live from /events/job-status</p>
			</article>
		</section>

		<section class="panel settings-panel">
			<h2>Appearance Settings</h2>
			{#if settings}
				<div class="settings-grid">
					<label>
						<span>Theme Mode</span>
						<select bind:value={settings.theme_mode}>
							<option value="system">System</option>
							<option value="dark">Dark</option>
							<option value="light">Light</option>
						</select>
					</label>
					<label>
						<span>Mobile Breakpoint (px)</span>
						<input bind:value={settings.mobile_breakpoint_px} type="number" min="320" max="1440" />
					</label>
					<label>
						<span>Bulk Selection Limit</span>
						<input bind:value={settings.bulk_selection_limit} type="number" min="10" max="1000" />
					</label>
					<label>
						<span>Max Filter Clauses</span>
						<input bind:value={settings.max_filter_clauses} type="number" min="2" max="50" />
					</label>
					<label>
						<span>Filter History Limit</span>
						<input bind:value={settings.filter_history_limit} type="number" min="1" max="100" />
					</label>
					<label>
						<span>Default Filter Operator</span>
						<select bind:value={settings.default_filter_operator}>
							<option value="and">AND</option>
							<option value="or">OR</option>
						</select>
					</label>
				</div>

				<div class="toggles">
					<label><input bind:checked={settings.mobile_compact_layout} type="checkbox" /> Compact mobile layout</label>
					<label><input bind:checked={settings.touch_targets_optimized} type="checkbox" /> Optimized touch targets</label>
					<label><input bind:checked={settings.keyboard_shortcuts_enabled} type="checkbox" /> Keyboard shortcuts</label>
					<label><input bind:checked={settings.bulk_operations_enabled} type="checkbox" /> Bulk operations</label>
					<label><input bind:checked={settings.bulk_action_confirmation} type="checkbox" /> Bulk action confirmation</label>
					<label><input bind:checked={settings.advanced_filtering_enabled} type="checkbox" /> Advanced filtering</label>
					<label><input bind:checked={settings.filter_history_enabled} type="checkbox" /> Filter history</label>
				</div>

				<button class="primary" type="button" onclick={saveSettings} disabled={busy}>
					{busy ? 'Saving…' : 'Save Settings'}
				</button>
			{:else}
				<p>Loading settings…</p>
			{/if}

			{#if settingsSaved}
				<p class="success">{settingsSaved}</p>
			{/if}
			{#if settingsError}
				<p class="error">{settingsError}</p>
			{/if}
		</section>
	{/if}
</main>
