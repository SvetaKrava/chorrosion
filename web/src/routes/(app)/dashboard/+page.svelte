<script lang="ts">
	import {
		ApiError,
		getAppearanceSettings,
		getProcessingSnapshot,
		getQueueSnapshot,
		getTasksSnapshot,
		sseUrl,
		updateAppearanceSettings
	} from '$lib/api';
	import type { AppearanceSettings, GenericListResponse } from '$lib/types';

	const DEFAULT_ERROR = 'Request failed. Please verify API server settings and try again.';

	let settings = $state<AppearanceSettings | null>(null);
	let settingsError = $state('');
	let settingsSaved = $state('');
	let saving = $state(false);

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
				streamState = 'reconnecting';
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

	async function saveSettings(): Promise<void> {
		if (!settings || saving) return;
		saving = true;
		settingsSaved = '';
		settingsError = '';
		try {
			settings = await updateAppearanceSettings(settings);
			settingsSaved = 'Appearance settings saved.';
		} catch (error) {
			settingsError = apiErrorMessage(error);
		} finally {
			saving = false;
		}
	}

	$effect(() => {
		hydrateData();
		attachSse();
		return () => detachSse();
	});
</script>

<section class="dashboard-section">
	<header class="section-header">
		<h2>Dashboard</h2>
		<div class="status-bar">
			<span class="pill" class:connected={streamState === 'connected'} class:reconnecting={streamState === 'reconnecting'}>
				{streamState}
			</span>
			<span class="pulse">Pulse: {pulse} #{pulseTick}</span>
		</div>
	</header>

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

			<button class="primary" type="button" onclick={saveSettings} disabled={saving}>
				{saving ? 'Saving…' : 'Save Settings'}
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
</section>

<style>
	.dashboard-section {
		display: flex;
		flex-direction: column;
		gap: 2rem;
	}

	.section-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 1rem;
	}

	.section-header h2 {
		margin: 0;
	}

	.status-bar {
		display: flex;
		gap: 1rem;
		align-items: center;
		font-size: 0.875rem;
	}

	.pill {
		display: inline-block;
		padding: 0.25rem 0.75rem;
		background: var(--bg-secondary);
		border: 1px solid var(--border-color);
		border-radius: 1rem;
		font-size: 0.75rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--text-secondary);
	}

	.pill.connected {
		background: rgba(var(--success-rgb), 0.1);
		border-color: var(--success);
		color: var(--success);
	}

	.pill.reconnecting {
		background: rgba(var(--warning-rgb), 0.1);
		border-color: var(--warning);
		color: var(--warning);
	}

	.pulse {
		color: var(--text-secondary);
	}

	.dashboard-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
		gap: 1.5rem;
	}

	.stat-panel {
		display: flex;
		flex-direction: column;
		justify-content: center;
		align-items: center;
		padding: 2rem;
		text-align: center;
	}

	.stat-panel h3 {
		margin: 0 0 1rem 0;
		font-size: 0.875rem;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--text-secondary);
	}

	.stat {
		margin: 0 0 0.5rem 0;
		font-size: 3rem;
		font-weight: 700;
		line-height: 1;
	}

	.meta {
		margin: 0;
		font-size: 0.75rem;
		color: var(--text-secondary);
	}

	.settings-panel {
		background: var(--bg-secondary);
		border: 1px solid var(--border-color);
		border-radius: 0.5rem;
		padding: 2rem;
	}

	.settings-panel h2 {
		margin: 0 0 1.5rem 0;
	}

	.settings-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
		gap: 1.5rem;
		margin-bottom: 1.5rem;
	}

	.toggles {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		margin-bottom: 1.5rem;
	}

	label {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	label > span {
		font-weight: 500;
		font-size: 0.875rem;
	}

	.toggles label {
		flex-direction: row;
		align-items: center;
		gap: 0.5rem;
	}

	.toggles input[type='checkbox'] {
		margin: 0;
	}

	input[type='number'],
	select {
		padding: 0.75rem;
		border: 1px solid var(--border-color);
		border-radius: 0.25rem;
		font-size: 1rem;
		background: var(--bg-primary);
		color: var(--text-primary);
	}

	input:focus,
	select:focus {
		outline: none;
		border-color: var(--accent);
		box-shadow: 0 0 0 2px rgba(var(--accent-rgb), 0.1);
	}

	button {
		padding: 0.75rem 1.5rem;
		border: none;
		border-radius: 0.25rem;
		font-weight: 600;
		font-size: 1rem;
		cursor: pointer;
		transition: all 0.2s;
		background: var(--accent);
		color: white;
	}

	button:hover {
		opacity: 0.9;
		transform: translateY(-1px);
	}

	button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.success {
		color: var(--success);
		margin: 1rem 0 0 0;
		padding: 0.75rem;
		background: rgba(var(--success-rgb), 0.1);
		border-radius: 0.25rem;
	}

	.error {
		color: var(--error);
		margin: 1rem 0 0 0;
		padding: 0.75rem;
		background: rgba(var(--error-rgb), 0.1);
		border-radius: 0.25rem;
	}
</style>
