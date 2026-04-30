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
	import type {
		ActivityItem,
		ActivityListResponse,
		AppearanceSettings,
		SystemTask,
		SystemTasksResponse
	} from '$lib/types';

	const DEFAULT_ERROR = 'Request failed. Please verify API server settings and try again.';

	let settings = $state<AppearanceSettings | null>(null);
	let settingsError = $state('');
	let settingsSaved = $state('');
	let saving = $state(false);

	let queueItems = $state<ActivityItem[]>([]);
	let queueTotal = $state(0);
	let processingItems = $state<ActivityItem[]>([]);
	let processingTotal = $state(0);
	let taskItems = $state<SystemTask[]>([]);
	let taskTotal = $state(0);
	let maxConcurrentJobs = $state(0);

	let queueUpdated = $state<Date | null>(null);
	let processingUpdated = $state<Date | null>(null);
	let tasksUpdated = $state<Date | null>(null);

	let pulse = $state('idle');
	let pulseTick = $state(0);

	type StreamConnectionState = 'connecting' | 'connected' | 'reconnecting' | 'disconnected';
	type StreamKey = 'events' | 'queue' | 'processing' | 'tasks';

	const ALL_STREAM_KEYS: StreamKey[] = ['events', 'queue', 'processing', 'tasks'];

	let streamStates = $state<Record<StreamKey, StreamConnectionState>>({
		events: 'disconnected',
		queue: 'disconnected',
		processing: 'disconnected',
		tasks: 'disconnected'
	});

	let streamState = $derived.by(() => {
		const states = ALL_STREAM_KEYS.map((k) => streamStates[k]);
		if (states.every((s) => s === 'connected')) return 'connected' as const;
		if (states.every((s) => s === 'disconnected')) return 'disconnected' as const;
		return 'reconnecting' as const;
	});

	let eventStreams = $state<Partial<Record<StreamKey, EventSource>>>({});
	let reconnectTimers = $state<Partial<Record<StreamKey, ReturnType<typeof setTimeout>>>>({});
	let reconnectAttempts = $state<Partial<Record<StreamKey, number>>>({});

	const RECONNECT_BASE_MS = 1000;
	const RECONNECT_MAX_MS = 30_000;
	const STALE_THRESHOLD_MS = 60_000;

	let now = $state(Date.now());
	let clockInterval: ReturnType<typeof setInterval> | null = null;

	const safeParse = (input: string): unknown => {
		try {
			return JSON.parse(input);
		} catch {
			return null;
		}
	};

	function apiErrorMessage(error: unknown): string {
		if (error instanceof ApiError) return error.message;
		if (error instanceof Error) return error.message;
		return DEFAULT_ERROR;
	}

	function backoffMs(key: StreamKey): number {
		const attempts = reconnectAttempts[key] ?? 0;
		return Math.min(RECONNECT_BASE_MS * 2 ** attempts, RECONNECT_MAX_MS);
	}

	function scheduleReconnect(key: StreamKey): void {
		clearTimeout(reconnectTimers[key]);
		const delay = backoffMs(key);
		reconnectAttempts[key] = (reconnectAttempts[key] ?? 0) + 1;
		streamStates[key] = 'reconnecting';
		reconnectTimers[key] = setTimeout(() => attachStream(key), delay);
	}

	function openStream(path: string): EventSource {
		return new EventSource(sseUrl(path), { withCredentials: true });
	}

	function attachStream(key: StreamKey): void {
		eventStreams[key]?.close();
		streamStates[key] = 'connecting';

		let es: EventSource;

		if (key === 'events') {
			es = openStream('/api/v1/events');
			es.addEventListener('connected', () => {
				reconnectAttempts[key] = 0;
				streamStates[key] = 'connected';
			});
			for (const evtName of ['download_progress', 'import_progress', 'job_status']) {
				es.addEventListener(evtName, (event) => {
					const payload = safeParse((event as MessageEvent).data) as {
						status?: string;
						tick?: number;
					} | null;
					if (payload?.status) pulse = payload.status;
					if (payload?.tick !== undefined) pulseTick = payload.tick;
				});
			}
		} else if (key === 'queue') {
			es = openStream('/api/v1/events/download-progress');
			es.addEventListener('connected', () => {
				reconnectAttempts[key] = 0;
				streamStates[key] = 'connected';
			});
			es.addEventListener('download_queue_snapshot', (event) => {
				const payload = safeParse((event as MessageEvent).data) as {
					queue?: ActivityListResponse;
				} | null;
				if (payload?.queue) {
					queueItems = payload.queue.items;
					queueTotal = payload.queue.total;
					queueUpdated = new Date();
				}
			});
		} else if (key === 'processing') {
			es = openStream('/api/v1/events/import-progress');
			es.addEventListener('connected', () => {
				reconnectAttempts[key] = 0;
				streamStates[key] = 'connected';
			});
			es.addEventListener('import_progress_snapshot', (event) => {
				const payload = safeParse((event as MessageEvent).data) as {
					processing?: ActivityListResponse;
				} | null;
				if (payload?.processing) {
					processingItems = payload.processing.items;
					processingTotal = payload.processing.total;
					processingUpdated = new Date();
				}
			});
		} else {
			es = openStream('/api/v1/events/job-status');
			es.addEventListener('connected', () => {
				reconnectAttempts[key] = 0;
				streamStates[key] = 'connected';
			});
			es.addEventListener('job_status_snapshot', (event) => {
				const payload = safeParse((event as MessageEvent).data) as {
					tasks?: SystemTasksResponse;
				} | null;
				if (payload?.tasks) {
					taskItems = payload.tasks.items;
					taskTotal = payload.tasks.total;
					maxConcurrentJobs = payload.tasks.max_concurrent_jobs;
					tasksUpdated = new Date();
				}
			});
		}

		es.onerror = () => {
			es.close();
			delete eventStreams[key];
			scheduleReconnect(key);
		};

		eventStreams[key] = es;
	}

	function attachSse(): void {
		for (const key of ALL_STREAM_KEYS) {
			attachStream(key);
		}
	}

	function detachSse(): void {
		for (const key of ALL_STREAM_KEYS) {
			clearTimeout(reconnectTimers[key]);
			eventStreams[key]?.close();
		}
		eventStreams = {};
		reconnectTimers = {};
		reconnectAttempts = {};
		streamStates = {
			events: 'disconnected',
			queue: 'disconnected',
			processing: 'disconnected',
			tasks: 'disconnected'
		};
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
			queueItems = queueSnapshot.items;
			queueTotal = queueSnapshot.total;
			queueUpdated = new Date();
			processingItems = processingSnapshot.items;
			processingTotal = processingSnapshot.total;
			processingUpdated = new Date();
			taskItems = tasksSnapshot.items;
			taskTotal = tasksSnapshot.total;
			maxConcurrentJobs = tasksSnapshot.max_concurrent_jobs;
			tasksUpdated = new Date();
		} catch (error) {
			settingsError = apiErrorMessage(error);
		}
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
		clockInterval = setInterval(() => {
			now = Date.now();
		}, 5000);
		return () => {
			detachSse();
			if (clockInterval) clearInterval(clockInterval);
		};
	});

	function formatAge(date: Date | null): string {
		if (!date) return 'never';
		const secs = Math.floor((now - date.getTime()) / 1000);
		if (secs < 5) return 'just now';
		if (secs < 60) return `${secs}s ago`;
		const mins = Math.floor(secs / 60);
		if (mins < 60) return `${mins}m ago`;
		return `${Math.floor(mins / 60)}h ago`;
	}

	function isStale(date: Date | null): boolean {
		if (!date) return true;
		return now - date.getTime() > STALE_THRESHOLD_MS;
	}

	function stateColor(state: string): string {
		switch (state) {
			case 'downloading':
				return 'state-active';
			case 'queued':
				return 'state-queued';
			case 'paused':
				return 'state-paused';
			case 'completed':
				return 'state-done';
			case 'error':
				return 'state-error';
			default:
				return 'state-unknown';
		}
	}

	function scheduleSummary(seconds: number): string {
		if (seconds < 60) return `Every ${seconds}s`;
		if (seconds < 3600) return `Every ${Math.round(seconds / 60)}m`;
		return `Every ${Math.round(seconds / 3600)}h`;
	}
</script>

<section class="dashboard-section">
	<header class="section-header">
		<h2>Dashboard</h2>
		<div class="status-bar">
			<span
				class="pill"
				class:connected={streamState === 'connected'}
				class:reconnecting={streamState === 'reconnecting'}
				class:disconnected={streamState === 'disconnected'}
			>
				{streamState}
			</span>
			<span class="pulse-label">Pulse: {pulse} #{pulseTick}</span>
		</div>
	</header>

	{#if streamState === 'disconnected'}
		<div class="degraded-banner">
			Realtime feed disconnected — data may be out of date.
		</div>
	{/if}

	<section class="activity-grid">
		<!-- Download Queue -->
		<article class="panel activity-panel" class:stale={isStale(queueUpdated)}>
			<div class="panel-header">
				<h3>Download Queue</h3>
				<span class="count-badge">{queueTotal}</span>
			</div>
			{#if queueUpdated}
				<p class="updated-at" class:stale-text={isStale(queueUpdated)}>
					Updated {formatAge(queueUpdated)}
				</p>
			{/if}
			{#if queueItems.length === 0}
				<p class="empty-state">No active downloads</p>
			{:else}
				<ul class="item-list">
					{#each queueItems as item (item.id)}
						<li class="activity-item">
							<div class="item-top">
								<span class="item-name" title={item.name}>{item.name}</span>
								<span class="state-badge {stateColor(item.state)}">{item.state}</span>
							</div>
							{#if item.state === 'downloading'}
								<div class="progress-bar" role="progressbar" aria-valuenow={item.progress_percent} aria-valuemin={0} aria-valuemax={100}>
									<div class="progress-fill" style="width: {item.progress_percent}%"></div>
								</div>
								<span class="progress-pct">{item.progress_percent}%</span>
							{/if}
						</li>
					{/each}
				</ul>
			{/if}
		</article>

		<!-- Import Processing -->
		<article class="panel activity-panel" class:stale={isStale(processingUpdated)}>
			<div class="panel-header">
				<h3>Import Processing</h3>
				<span class="count-badge">{processingTotal}</span>
			</div>
			{#if processingUpdated}
				<p class="updated-at" class:stale-text={isStale(processingUpdated)}>
					Updated {formatAge(processingUpdated)}
				</p>
			{/if}
			{#if processingItems.length === 0}
				<p class="empty-state">No active imports</p>
			{:else}
				<ul class="item-list">
					{#each processingItems as item (item.id)}
						<li class="activity-item">
							<div class="item-top">
								<span class="item-name" title={item.name}>{item.name}</span>
								<span class="state-badge {stateColor(item.state)}">{item.state}</span>
							</div>
							{#if item.progress_percent > 0}
								<div class="progress-bar" role="progressbar" aria-valuenow={item.progress_percent} aria-valuemin={0} aria-valuemax={100}>
									<div class="progress-fill" style="width: {item.progress_percent}%"></div>
								</div>
								<span class="progress-pct">{item.progress_percent}%</span>
							{/if}
						</li>
					{/each}
				</ul>
			{/if}
		</article>

		<!-- Scheduled Tasks -->
		<article class="panel activity-panel" class:stale={isStale(tasksUpdated)}>
			<div class="panel-header">
				<h3>Scheduled Tasks</h3>
				<span class="count-badge">{taskTotal}</span>
				{#if maxConcurrentJobs > 0}
					<span class="concurrency-label">max {maxConcurrentJobs} concurrent</span>
				{/if}
			</div>
			{#if tasksUpdated}
				<p class="updated-at" class:stale-text={isStale(tasksUpdated)}>
					Updated {formatAge(tasksUpdated)}
				</p>
			{/if}
			{#if taskItems.length === 0}
				<p class="empty-state">No scheduled tasks</p>
			{:else}
				<ul class="task-list">
					{#each taskItems as task (task.id)}
						<li class="task-item">
							<div class="task-info">
								<span class="task-name">{task.name}</span>
								<span class="task-schedule">{scheduleSummary(task.schedule_seconds)}</span>
							</div>
							<div class="task-meta">
								<span class="state-badge {task.enabled ? 'state-active' : 'state-paused'}">
									{task.enabled ? 'enabled' : 'disabled'}
								</span>
								<span class="state-badge {stateColor(task.status)}">{task.status}</span>
							</div>
						</li>
					{/each}
				</ul>
			{/if}
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
		flex-wrap: wrap;
		gap: 0.75rem;
	}

	.section-header h2 {
		margin: 0;
	}

	.status-bar {
		display: flex;
		gap: 0.75rem;
		align-items: center;
		font-size: 0.875rem;
		flex-wrap: wrap;
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

	.pill.disconnected {
		background: rgba(var(--error-rgb), 0.1);
		border-color: var(--error);
		color: var(--error);
	}

	.pulse-label {
		color: var(--text-secondary);
		font-size: 0.8rem;
	}

	.degraded-banner {
		padding: 0.75rem 1rem;
		background: rgba(var(--error-rgb), 0.08);
		border: 1px solid var(--error);
		border-radius: 0.375rem;
		color: var(--error);
		font-size: 0.875rem;
		font-weight: 500;
	}

	/* Activity grid — 3 columns on wide, stack on mobile */
	.activity-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
		gap: 1.5rem;
	}

	.activity-panel {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		min-width: 0; /* prevent overflow on mobile */
	}

	.activity-panel.stale {
		opacity: 0.7;
	}

	.panel-header {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex-wrap: wrap;
	}

	.panel-header h3 {
		margin: 0;
		font-size: 0.875rem;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--text-secondary);
		flex: 1;
	}

	.count-badge {
		display: inline-block;
		padding: 0.15rem 0.5rem;
		background: var(--bg-secondary);
		border: 1px solid var(--border-color);
		border-radius: 0.75rem;
		font-size: 0.75rem;
		font-weight: 700;
		color: var(--text-primary);
	}

	.concurrency-label {
		font-size: 0.7rem;
		color: var(--text-secondary);
	}

	.updated-at {
		margin: 0;
		font-size: 0.7rem;
		color: var(--text-secondary);
	}

	.updated-at.stale-text {
		color: var(--warning);
	}

	.empty-state {
		margin: 0.5rem 0;
		padding: 1.5rem;
		text-align: center;
		font-size: 0.875rem;
		color: var(--text-secondary);
		border: 1px dashed var(--border-color);
		border-radius: 0.375rem;
	}

	/* Item list (queue / processing) */
	.item-list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.activity-item {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		padding: 0.5rem 0.75rem;
		background: var(--bg-secondary);
		border: 1px solid var(--border-color);
		border-radius: 0.375rem;
	}

	.item-top {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		min-width: 0;
	}

	.item-name {
		flex: 1;
		font-size: 0.8rem;
		font-weight: 500;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.progress-bar {
		height: 4px;
		background: var(--border-color);
		border-radius: 2px;
		overflow: hidden;
	}

	.progress-fill {
		height: 100%;
		background: var(--accent);
		border-radius: 2px;
		transition: width 0.3s ease;
	}

	.progress-pct {
		font-size: 0.7rem;
		color: var(--text-secondary);
		text-align: right;
	}

	/* Task list */
	.task-list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.task-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		padding: 0.5rem 0.75rem;
		background: var(--bg-secondary);
		border: 1px solid var(--border-color);
		border-radius: 0.375rem;
		flex-wrap: wrap;
	}

	.task-info {
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
		min-width: 0;
		flex: 1;
	}

	.task-name {
		font-size: 0.8rem;
		font-weight: 500;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.task-schedule {
		font-size: 0.7rem;
		color: var(--text-secondary);
	}

	.task-meta {
		display: flex;
		gap: 0.25rem;
		flex-shrink: 0;
	}

	/* State badges */
	.state-badge {
		display: inline-block;
		padding: 0.15rem 0.45rem;
		border-radius: 0.25rem;
		font-size: 0.65rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		white-space: nowrap;
	}

	.state-active {
		background: rgba(var(--success-rgb), 0.15);
		color: var(--success);
	}

	.state-queued {
		background: rgba(var(--accent-rgb), 0.15);
		color: var(--accent);
	}

	.state-paused {
		background: rgba(var(--warning-rgb), 0.15);
		color: var(--warning);
	}

	.state-done {
		background: rgba(var(--success-rgb), 0.08);
		color: var(--text-secondary);
	}

	.state-error {
		background: rgba(var(--error-rgb), 0.15);
		color: var(--error);
	}

	.state-unknown {
		background: var(--bg-secondary);
		color: var(--text-secondary);
	}

	/* Settings panel */
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
