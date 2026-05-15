<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import SettingsSection from '$lib/components/settings/SettingsSection.svelte';
	import EmptyState from '$lib/components/settings/EmptyState.svelte';
	import LoadingSpinner from '$lib/components/settings/LoadingSpinner.svelte';
	import ErrorMessage from '$lib/components/settings/ErrorMessage.svelte';
	import ConfirmDialog from '$lib/components/settings/ConfirmDialog.svelte';
	import ImportPreviewDialog from '$lib/components/settings/ImportPreviewDialog.svelte';
	import SaveStatusBanner from '$lib/components/settings/SaveStatusBanner.svelte';
	import type { SaveStatus } from '$lib/components/settings/SaveStatusBanner.svelte';
	import {
		getIndexers,
		createIndexer,
		updateIndexer,
		deleteIndexer,
		bulkIndexers,
		exportIndexers,
		importIndexers,
		testIndexer,
		ApiError
	} from '$lib/api';
	import { useUnsavedGuard } from '$lib/stores/unsavedGuard';
	import { classifyFormError } from '$lib/settingsValidation';
	import type {
		Indexer,
		CreateIndexerRequest,
		SettingsImportConflictPolicy,
		SettingsImportResponse,
		UpdateIndexerRequest,
		TestIndexerResponse
	} from '$lib/types';

	// ── state ──────────────────────────────────────────────────────────────────
	let indexers = $state<Indexer[]>([]);
	let selectedIds = $state<Set<string>>(new Set());
	let loading = $state(true);
	let loadError = $state('');
	let saveStatus = $state<SaveStatus>('idle');
	let saveError = $state('');
	let formDirty = $state(false);
	let leaveDialogOpen = $state(false);
	let initialFormSnapshot = '';

	// modal state
	let modalOpen = $state(false);
	let editingIndexer = $state<Indexer | null>(null);

	// form fields
	let formName = $state('');
	let formProtocol = $state('torznab');
	let formBaseUrl = $state('');
	let formApiKey = $state('');
	let formEnabled = $state(true);
	let formErrors = $state<Record<string, string>>({});
	let formSaving = $state(false);

	// test state
	let testStatus = $state<'idle' | 'testing' | 'success' | 'error'>('idle');
	let testResult = $state<TestIndexerResponse | null>(null);
	let testError = $state('');

	// delete state
	let deleteTarget = $state<Indexer | null>(null);
	let deleteDialogOpen = $state(false);
	let deleting = $state(false);
	let bulkDeleteDialogOpen = $state(false);
	let bulkActing = $state(false);
	let importDialogOpen = $state(false);
	let importApplying = $state(false);
	let importPolicy = $state<SettingsImportConflictPolicy>('merge');
	let importVersion = $state('1');
	let importItems = $state<CreateIndexerRequest[]>([]);
	let importPreview = $state<SettingsImportResponse | null>(null);
	let importFileInput: HTMLInputElement | null = null;

	// banner auto-clear timer
	let saveStatusTimer: ReturnType<typeof setTimeout> | null = null;
	const unsavedGuard = useUnsavedGuard(() => {
		leaveDialogOpen = true;
	});

	function getFormSnapshot(): string {
		return JSON.stringify({
			name: formName.trim(),
			protocol: formProtocol,
			baseUrl: formBaseUrl.trim(),
			apiKey: formApiKey,
			enabled: formEnabled
		});
	}

	function syncDirtyState() {
		formDirty = getFormSnapshot() !== initialFormSnapshot;
		if (formDirty) {
			unsavedGuard.markDirty();
		} else {
			unsavedGuard.markClean();
		}
	}

	function clearFieldError(field: string) {
		if (formErrors[field]) {
			const { [field]: _removed, ...rest } = formErrors;
			formErrors = rest;
		}
	}

	function scheduleBannerClear() {
		if (saveStatusTimer !== null) clearTimeout(saveStatusTimer);
		saveStatusTimer = setTimeout(() => {
			saveStatus = 'idle';
			saveStatusTimer = null;
		}, 2500);
	}

	onDestroy(() => {
		if (saveStatusTimer !== null) clearTimeout(saveStatusTimer);
	});

	// ── constants ──────────────────────────────────────────────────────────────
	const PROTOCOLS = [
		{ value: 'torznab', label: 'Torznab' },
		{ value: 'newznab', label: 'Newznab' },
		{ value: 'gazelle', label: 'Gazelle' }
	];

	// ── data loading ───────────────────────────────────────────────────────────
	async function load() {
		loading = true;
		loadError = '';
		try {
			const result = await getIndexers({ limit: 100 });
			indexers = result.items;
		} catch (err) {
			loadError = err instanceof ApiError ? err.message : 'Failed to load indexers.';
		} finally {
			loading = false;
		}
	}

	onMount(load);

	let hasSelection = $derived(selectedIds.size > 0);
	let allSelected = $derived(indexers.length > 0 && indexers.every((indexer) => selectedIds.has(indexer.id)));

	// ── modal helpers ──────────────────────────────────────────────────────────
	function openAdd() {
		editingIndexer = null;
		formName = '';
		formProtocol = 'torznab';
		formBaseUrl = '';
		formApiKey = '';
		formEnabled = true;
		formErrors = {};
		testStatus = 'idle';
		testResult = null;
		testError = '';
		leaveDialogOpen = false;
		modalOpen = true;
		initialFormSnapshot = getFormSnapshot();
		syncDirtyState();
	}

	function openEdit(indexer: Indexer) {
		editingIndexer = indexer;
		formName = indexer.name;
		formProtocol = indexer.protocol;
		formBaseUrl = indexer.base_url;
		formApiKey = '';
		formEnabled = indexer.enabled;
		formErrors = {};
		testStatus = 'idle';
		testResult = null;
		testError = '';
		leaveDialogOpen = false;
		modalOpen = true;
		initialFormSnapshot = getFormSnapshot();
		syncDirtyState();
	}

	function closeModal() {
		if (formDirty) {
			leaveDialogOpen = true;
			return;
		}
		unsavedGuard.discardNavigation();
		modalOpen = false;
		editingIndexer = null;
	}

	function validateForm(): boolean {
		const errors: Record<string, string> = {};
		if (!formName.trim()) errors.name = 'Name is required.';
		if (!formBaseUrl.trim()) {
			errors.base_url = 'Base URL is required.';
		} else {
			try {
				const u = new URL(formBaseUrl.trim());
				if (!['http:', 'https:'].includes(u.protocol)) {
					errors.base_url = 'URL must use http or https.';
				}
			} catch {
				errors.base_url = 'Must be a valid URL (e.g. http://localhost:9117).';
			}
		}
		formErrors = errors;
		return Object.keys(errors).length === 0;
	}

	function testRequiresApiKey(): boolean {
		return !!editingIndexer?.has_api_key && !formApiKey.trim();
	}

	async function saveForm() {
		if (!validateForm()) return;
		formSaving = true;
		saveStatus = 'saving';
		saveError = '';
		try {
			if (editingIndexer) {
				const payload: UpdateIndexerRequest = {
					name: formName.trim(),
					protocol: formProtocol,
					base_url: formBaseUrl.trim(),
					api_key: formApiKey || undefined,
					enabled: formEnabled
				};
				const updated = await updateIndexer(editingIndexer.id, payload);
				indexers = indexers.map((i) => (i.id === updated.id ? updated : i));
			} else {
				const payload: CreateIndexerRequest = {
					name: formName.trim(),
					protocol: formProtocol,
					base_url: formBaseUrl.trim(),
					api_key: formApiKey || null,
					enabled: formEnabled
				};
				const created = await createIndexer(payload);
				indexers = [...indexers, created];
			}
			unsavedGuard.markClean();
			formDirty = false;
			initialFormSnapshot = getFormSnapshot();
			closeModal();
			saveStatus = 'saved';
			scheduleBannerClear();
		} catch (err) {
			const classified = classifyFormError(err, [
				{ field: 'name', messages: ['name cannot be empty', 'already exists'] },
				{
					field: 'base_url',
					messages: [
						'base_url must be a valid http or https URL with a host',
						'Indexer base_url must be a valid http or https URL with a host'
					]
				},
				{ field: 'protocol', messages: ['unsupported protocol', 'invalid value'] }
			]);
			if (Object.keys(classified.fieldErrors).length > 0) {
				formErrors = { ...formErrors, ...classified.fieldErrors };
				saveStatus = 'idle';
				saveError = '';
			} else {
				saveError = classified.bannerMessage || 'Save failed.';
				saveStatus = 'error';
			}
		} finally {
			formSaving = false;
		}
	}

	async function runTest() {
		if (!validateForm()) return;
		if (testRequiresApiKey()) {
			testStatus = 'error';
			testResult = null;
			testError = 'Enter the API key to test this indexer.';
			return;
		}
		testStatus = 'testing';
		testResult = null;
		testError = '';
		try {
			const result = await testIndexer({
				name: formName.trim(),
				base_url: formBaseUrl.trim(),
				protocol: formProtocol,
				api_key: formApiKey || null
			});
			testResult = result;
			testStatus = result.success ? 'success' : 'error';
			if (!result.success) testError = result.message;
		} catch (err) {
			testStatus = 'error';
			testError = err instanceof ApiError ? err.message : 'Test failed.';
		}
	}

	// ── toggle enabled ─────────────────────────────────────────────────────────
	async function toggleEnabled(indexer: Indexer) {
		try {
			const updated = await updateIndexer(indexer.id, { enabled: !indexer.enabled });
			indexers = indexers.map((i) => (i.id === updated.id ? updated : i));
		} catch (err) {
			saveError = err instanceof ApiError ? err.message : 'Failed to update indexer.';
			saveStatus = 'error';
		}
	}

	async function exportItems() {
		try {
			const payload = await exportIndexers();
			const blob = new Blob([JSON.stringify(payload, null, 2)], { type: 'application/json' });
			const url = URL.createObjectURL(blob);
			const link = document.createElement('a');
			link.href = url;
			link.download = 'indexers.export.json';
			link.click();
			URL.revokeObjectURL(url);
		} catch (err) {
			saveError = err instanceof ApiError ? err.message : 'Export failed.';
			saveStatus = 'error';
		}
	}

	function triggerImport() {
		importFileInput?.click();
	}

	async function refreshImportPreview() {
		importPreview = await importIndexers(
			{
				version: importVersion,
				conflict_policy: importPolicy,
				items: importItems
			},
			true
		);
	}

	async function handleImportFile(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;
		try {
			const text = await file.text();
			const parsed = JSON.parse(text);
			importVersion = typeof parsed.version === 'string' ? parsed.version : '1';
			importItems = Array.isArray(parsed.items)
				? parsed.items
				: Array.isArray(parsed)
					? parsed
					: [];
			await refreshImportPreview();
			importDialogOpen = true;
		} catch (err) {
			saveError = err instanceof Error ? err.message : 'Import file is invalid.';
			saveStatus = 'error';
		}
		input.value = '';
	}

	async function applyImport() {
		importApplying = true;
		try {
			const result = await importIndexers(
				{
					version: importVersion,
					conflict_policy: importPolicy,
					items: importItems
				},
				false
			);
			const failed = result.results.filter((item) => !item.success);
			if (failed.length > 0) {
				saveError = failed
					.slice(0, 4)
					.map((item) => `${item.id}: ${item.error ?? 'operation failed'}`)
					.join('\n');
				saveStatus = 'error';
			} else {
				saveStatus = 'saved';
				scheduleBannerClear();
			}
			importDialogOpen = false;
			await load();
		} catch (err) {
			saveError = err instanceof ApiError ? err.message : 'Import failed.';
			saveStatus = 'error';
		} finally {
			importApplying = false;
		}
	}

	function toggleRowSelection(id: string) {
		const next = new Set(selectedIds);
		if (next.has(id)) {
			next.delete(id);
		} else {
			next.add(id);
		}
		selectedIds = next;
	}

	function toggleSelectAll() {
		if (allSelected) {
			selectedIds = new Set();
			return;
		}
		selectedIds = new Set(indexers.map((indexer) => indexer.id));
	}

	async function runBulkAction(action: 'enable' | 'disable' | 'delete') {
		if (selectedIds.size === 0) return;

		const ids = [...selectedIds];
		const selectedSet = new Set(ids);
		const originalIndexers = indexers;
		const originalById = new Map(originalIndexers.map((indexer) => [indexer.id, indexer]));

		if (action === 'delete') {
			indexers = indexers.filter((indexer) => !selectedSet.has(indexer.id));
		} else {
			indexers = indexers.map((indexer) =>
				selectedSet.has(indexer.id)
					? {
						...indexer,
						enabled: action === 'enable'
					}
					: indexer
			);
		}

		saveStatus = 'saving';
		saveError = '';
		bulkActing = true;

		try {
			const result = await bulkIndexers({ action, ids });
			const failed = result.results.filter((item) => !item.success);
			if (failed.length > 0) {
				const failedSet = new Set(failed.map((item) => item.id));
				if (action === 'delete') {
					indexers = originalIndexers.filter(
						(indexer) => !selectedSet.has(indexer.id) || failedSet.has(indexer.id)
					);
				} else {
					indexers = indexers.map((indexer) =>
						failedSet.has(indexer.id) ? (originalById.get(indexer.id) ?? indexer) : indexer
					);
				}

				selectedIds = failedSet;
				saveStatus = 'error';
				saveError = failed
					.slice(0, 4)
					.map((item) => `${item.id}: ${item.error ?? 'operation failed'}`)
					.join('\n');
				return;
			}

			selectedIds = new Set();
			saveStatus = 'saved';
			scheduleBannerClear();
		} catch (err) {
			indexers = originalIndexers;
			saveStatus = 'error';
			saveError = err instanceof ApiError ? err.message : 'Bulk action failed.';
		} finally {
			bulkActing = false;
		}
	}

	function openBulkDelete() {
		if (!hasSelection) return;
		bulkDeleteDialogOpen = true;
	}

	async function confirmBulkDelete() {
		await runBulkAction('delete');
		bulkDeleteDialogOpen = false;
	}

	// ── delete ─────────────────────────────────────────────────────────────────
	function openDelete(indexer: Indexer) {
		deleteTarget = indexer;
		deleteDialogOpen = true;
	}

	async function confirmDelete() {
		if (!deleteTarget) return;
		deleting = true;
		try {
			await deleteIndexer(deleteTarget.id);
			indexers = indexers.filter((i) => i.id !== deleteTarget!.id);
			saveStatus = 'saved';
			scheduleBannerClear();
		} catch (err) {
			saveError = err instanceof ApiError ? err.message : 'Delete failed.';
			saveStatus = 'error';
		} finally {
			deleting = false;
			deleteDialogOpen = false;
			deleteTarget = null;
		}
	}

	function cancelDelete() {
		deleteDialogOpen = false;
		deleteTarget = null;
	}

	function protocolLabel(protocol: string): string {
		return PROTOCOLS.find((p) => p.value === protocol)?.label ?? protocol;
	}

	function confirmLeave() {
		leaveDialogOpen = false;
		modalOpen = false;
		editingIndexer = null;
		void unsavedGuard.confirmNavigation();
	}

	function cancelLeave() {
		leaveDialogOpen = false;
		unsavedGuard.discardNavigation();
	}
</script>

<SettingsSection
	title="Indexers"
	description="Configure Torznab, Newznab, and Gazelle indexers used to search for releases."
>
	{#snippet actions()}
		<SaveStatusBanner status={saveStatus} errorMessage={saveError} />
		<button class="btn-secondary" onclick={exportItems}>Export</button>
		<button class="btn-secondary" onclick={triggerImport}>Import</button>
		<input bind:this={importFileInput} type="file" accept="application/json" hidden onchange={handleImportFile} />
		{#if hasSelection}
			<div class="bulk-toolbar">
				<button class="btn-secondary" onclick={() => runBulkAction('enable')} disabled={bulkActing}>
					Enable Selected
				</button>
				<button class="btn-secondary" onclick={() => runBulkAction('disable')} disabled={bulkActing}>
					Disable Selected
				</button>
				<button class="btn-icon destructive" onclick={openBulkDelete} disabled={bulkActing}>
					Delete Selected
				</button>
			</div>
		{/if}
		<button class="btn-primary" onclick={openAdd}>Add Indexer</button>
	{/snippet}

	{#if loading}
		<LoadingSpinner label="Loading indexers…" />
	{:else if loadError}
		<ErrorMessage message={loadError} onretry={load} />
	{:else if indexers.length === 0}
		<EmptyState
			message="No indexers configured."
			actionLabel="Add Indexer"
			onaction={openAdd}
		/>
	{:else}
		<div class="bulk-select-row">
			<label>
				<input type="checkbox" checked={allSelected} onchange={toggleSelectAll} />
				Select All ({selectedIds.size}/{indexers.length})
			</label>
		</div>
		<ul class="indexer-list" role="list">
			{#each indexers as indexer (indexer.id)}
				<li class="indexer-item" class:disabled={!indexer.enabled}>
					<input
						type="checkbox"
						checked={selectedIds.has(indexer.id)}
						onchange={() => toggleRowSelection(indexer.id)}
						aria-label="Select {indexer.name}"
					/>
					<div class="indexer-info">
						<span class="indexer-name">{indexer.name}</span>
						<span class="indexer-meta">
							{protocolLabel(indexer.protocol)} · {indexer.base_url}
							{#if indexer.has_api_key} · API key set{/if}
						</span>
					</div>
					<div class="indexer-actions">
						<label class="toggle" title={indexer.enabled ? 'Disable' : 'Enable'}>
							<input
								type="checkbox"
								checked={indexer.enabled}
								onchange={() => toggleEnabled(indexer)}
								aria-label="Enable {indexer.name}"
							/>
							<span class="toggle-track"></span>
						</label>
						<button class="btn-icon" onclick={() => openEdit(indexer)} aria-label="Edit {indexer.name}">
							Edit
						</button>
						<button
							class="btn-icon destructive"
							onclick={() => openDelete(indexer)}
							aria-label="Delete {indexer.name}"
						>
							Delete
						</button>
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</SettingsSection>

<!-- ── Add / Edit modal ─────────────────────────────────────────────────── -->
<svelte:window onkeydown={(e) => { if (e.key === 'Escape' && modalOpen) { e.preventDefault(); e.stopPropagation(); closeModal(); } }} />
{#if modalOpen}
	<div class="modal-backdrop" role="dialog" aria-modal="true" aria-labelledby="modal-title">
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<div class="modal-scrim" role="presentation" onclick={closeModal}></div>
		<div class="modal-panel">
			<h3 class="modal-title" id="modal-title">
				{editingIndexer ? 'Edit Indexer' : 'Add Indexer'}
			</h3>

			<form
				class="modal-form"
				onsubmit={(e) => {
					e.preventDefault();
					saveForm();
				}}
			>
				<div class="field" class:has-error={!!formErrors.name}>
					<label for="idx-name">Name <span aria-hidden="true">*</span></label>
					<input
						id="idx-name"
						type="text"
						bind:value={formName}
						placeholder="My Indexer"
						oninput={() => {
							clearFieldError('name');
							syncDirtyState();
						}}
					/>
					{#if formErrors.name}<span class="field-error">{formErrors.name}</span>{/if}
				</div>

				<div class="field" class:has-error={!!formErrors.protocol}>
					<label for="idx-protocol">Protocol</label>
					<select
						id="idx-protocol"
						bind:value={formProtocol}
						onchange={() => {
							clearFieldError('protocol');
							syncDirtyState();
						}}
					>
						{#each PROTOCOLS as p}
							<option value={p.value}>{p.label}</option>
						{/each}
					</select>
					{#if formErrors.protocol}<span class="field-error">{formErrors.protocol}</span>{/if}
				</div>

				<div class="field" class:has-error={!!formErrors.base_url}>
					<label for="idx-url">Base URL <span aria-hidden="true">*</span></label>
					<input
						id="idx-url"
						type="url"
						bind:value={formBaseUrl}
						placeholder="http://localhost:9117"
						oninput={() => {
							clearFieldError('base_url');
							syncDirtyState();
						}}
					/>
					{#if formErrors.base_url}<span class="field-error">{formErrors.base_url}</span>{/if}
				</div>

				<div class="field">
					<label for="idx-apikey">
						API Key
						{#if editingIndexer && editingIndexer.has_api_key}
							<span class="hint">(leave blank to keep existing)</span>
						{/if}
					</label>
					<input
						id="idx-apikey"
						type="password"
						bind:value={formApiKey}
						autocomplete="off"
						oninput={() => {
							clearFieldError('api_key');
							syncDirtyState();
						}}
					/>
				</div>

				<div class="field field-inline">
					<label for="idx-enabled">Enabled</label>
					<input
						id="idx-enabled"
						type="checkbox"
						bind:checked={formEnabled}
						onchange={() => {
							clearFieldError('enabled');
							syncDirtyState();
						}}
					/>
				</div>

				<!-- Test result banner -->
				{#if testStatus === 'success' && testResult}
					<div class="test-result success">
						<strong>Connection successful</strong> — {testResult.message}
						{#if
							testResult.capabilities.supports_search ||
							testResult.capabilities.supports_rss ||
							testResult.capabilities.supports_categories}
							<ul class="capabilities">
								{#if testResult.capabilities.supports_search}<li>Search</li>{/if}
								{#if testResult.capabilities.supports_rss}<li>RSS</li>{/if}
								{#if testResult.capabilities.supports_categories}<li>Categories</li>{/if}
							</ul>
						{/if}
					</div>
				{:else if testStatus === 'error'}
					<div class="test-result error">
						<strong>Test failed</strong> — {testError}
					</div>
				{/if}

				<div class="modal-actions">
					<button type="button" class="btn-cancel" onclick={closeModal}>Cancel</button>
					<button
						type="button"
						class="btn-secondary"
						onclick={runTest}
						disabled={testStatus === 'testing' || testRequiresApiKey()}
					>
						{testStatus === 'testing' ? 'Testing…' : 'Test'}
					</button>
					<button type="submit" class="btn-primary" disabled={formSaving || !formDirty}>
						{formSaving ? 'Saving…' : editingIndexer ? 'Save Changes' : 'Add Indexer'}
					</button>
				</div>
			</form>
		</div>
	</div>
{/if}

<ConfirmDialog
	open={leaveDialogOpen}
	title="Leave with unsaved changes?"
	message="You have unsaved changes. Leave anyway?"
	confirmLabel="Leave"
	destructive
	onconfirm={confirmLeave}
	oncancel={cancelLeave}
/>

<!-- ── Delete confirmation ──────────────────────────────────────────────── -->
<ConfirmDialog
	open={deleteDialogOpen}
	title="Delete Indexer"
	message={`Delete "${deleteTarget?.name ?? ''}"? This cannot be undone.`}
	confirmLabel={deleting ? 'Deleting…' : 'Delete'}
	destructive
	onconfirm={confirmDelete}
	oncancel={cancelDelete}
/>

<ConfirmDialog
	open={bulkDeleteDialogOpen}
	title="Delete Selected Indexers"
	message={`Delete ${selectedIds.size} selected indexer(s)? This cannot be undone.`}
	confirmLabel={bulkActing ? 'Deleting…' : 'Delete Selected'}
	destructive
	onconfirm={confirmBulkDelete}
	oncancel={() => (bulkDeleteDialogOpen = false)}
/>

<ImportPreviewDialog
	open={importDialogOpen}
	title="Preview Indexer Import"
	summary={importPreview?.summary ?? { added: 0, updated: 0, deleted: 0 }}
	preview={importPreview?.preview ?? []}
	policy={importPolicy}
	applying={importApplying}
	onPolicyChange={(policy) => {
		importPolicy = policy;
		void refreshImportPreview();
	}}
	onConfirm={applyImport}
	onCancel={() => (importDialogOpen = false)}
/>

<style>
	.indexer-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.bulk-toolbar {
		display: flex;
		align-items: center;
		gap: 0.4rem;
	}

	.bulk-select-row {
		margin-bottom: 0.6rem;
		font-size: 0.85rem;
	}

	.bulk-select-row label {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
	}

	.indexer-item {
		display: flex;
		align-items: center;
		gap: 1rem;
		padding: 0.85rem 1.1rem;
		background: var(--paper, #fffdf7);
		border: 1px solid var(--border-color, rgba(15, 26, 31, 0.12));
		border-radius: 8px;
		transition:
			border-color 0.15s,
			opacity 0.15s;
	}

	.indexer-item:hover {
		border-color: var(--accent, #0f7b6c);
	}

	.indexer-item.disabled {
		opacity: 0.6;
	}

	.indexer-info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
	}

	.indexer-name {
		font-weight: 600;
		font-size: 0.95rem;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.indexer-meta {
		font-size: 0.8rem;
		color: var(--text-secondary, #666);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.indexer-actions {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex-shrink: 0;
	}

	/* Toggle switch */
	.toggle {
		position: relative;
		display: inline-flex;
		align-items: center;
		cursor: pointer;
	}

	.toggle input {
		position: absolute;
		opacity: 0;
		width: 0;
		height: 0;
	}

	.toggle-track {
		display: inline-block;
		width: 2.25rem;
		height: 1.25rem;
		background: var(--border-color, #ccc);
		border-radius: 999px;
		transition: background 0.2s;
		position: relative;
	}

	.toggle-track::after {
		content: '';
		position: absolute;
		top: 2px;
		left: 2px;
		width: 1rem;
		height: 1rem;
		background: #fff;
		border-radius: 50%;
		transition: transform 0.2s;
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
	}

	.toggle input:checked + .toggle-track {
		background: var(--accent, #0f7b6c);
	}

	.toggle input:checked + .toggle-track::after {
		transform: translateX(1rem);
	}

	.toggle input:focus-visible + .toggle-track {
		outline: 2px solid var(--accent, #0f7b6c);
		outline-offset: 2px;
	}

	/* Icon buttons */
	.btn-icon {
		padding: 0.35rem 0.7rem;
		border: 1px solid var(--border-color, rgba(15, 26, 31, 0.2));
		border-radius: 5px;
		background: transparent;
		font-size: 0.8rem;
		cursor: pointer;
		color: var(--text-primary, #0f1a1f);
		transition:
			background 0.15s,
			border-color 0.15s;
	}

	.btn-icon:hover {
		background: var(--hover-bg, rgba(15, 26, 31, 0.06));
		border-color: var(--accent, #0f7b6c);
	}

	.btn-icon.destructive:hover {
		background: rgba(192, 57, 43, 0.08);
		border-color: #c0392b;
		color: #c0392b;
	}

	/* Modal */
	.modal-backdrop {
		position: fixed;
		inset: 0;
		z-index: 50;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.modal-scrim {
		position: absolute;
		inset: 0;
		background: rgba(0, 0, 0, 0.4);
	}

	.modal-panel {
		position: relative;
		background: var(--paper, #fffdf7);
		border-radius: 12px;
		padding: 1.75rem 2rem;
		width: min(480px, 95vw);
		max-height: 90vh;
		overflow-y: auto;
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.18);
		animation: pop-in 0.15s ease;
	}

	@keyframes pop-in {
		from {
			opacity: 0;
			transform: scale(0.95);
		}
		to {
			opacity: 1;
			transform: scale(1);
		}
	}

	.modal-title {
		margin: 0 0 1.25rem;
		font-size: 1.1rem;
		font-weight: 700;
	}

	.modal-form {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
	}

	.field label {
		font-size: 0.85rem;
		font-weight: 600;
		color: var(--text-primary, #0f1a1f);
	}

	.field input[type='text'],
	.field input[type='url'],
	.field input[type='password'],
	.field select {
		padding: 0.5rem 0.75rem;
		border: 1px solid var(--border-color, rgba(15, 26, 31, 0.2));
		border-radius: 6px;
		font-size: 0.9rem;
		background: var(--surface, #fff);
		color: var(--text-primary, #0f1a1f);
		transition: border-color 0.15s;
	}

	.field input:focus,
	.field select:focus {
		outline: none;
		border-color: var(--accent, #0f7b6c);
	}	.field.has-error input {
		border-color: #c0392b;
	}

	.field-error {
		font-size: 0.78rem;
		color: #c0392b;
	}

	.field-inline {
		flex-direction: row;
		align-items: center;
		gap: 0.75rem;
	}

	.field-inline input[type='checkbox'] {
		width: 1rem;
		height: 1rem;
		accent-color: var(--accent, #0f7b6c);
	}

	.hint {
		font-weight: 400;
		font-size: 0.78rem;
		color: var(--text-secondary, #666);
	}

	/* Test result */
	.test-result {
		padding: 0.65rem 0.9rem;
		border-radius: 6px;
		font-size: 0.85rem;
		line-height: 1.4;
	}

	.test-result.success {
		background: rgba(15, 123, 108, 0.1);
		border: 1px solid rgba(15, 123, 108, 0.3);
		color: var(--text-primary, #0f1a1f);
	}

	.test-result.error {
		background: rgba(192, 57, 43, 0.08);
		border: 1px solid rgba(192, 57, 43, 0.3);
		color: #c0392b;
	}

	.capabilities {
		margin: 0.35rem 0 0;
		padding: 0;
		list-style: none;
		display: flex;
		gap: 0.5rem;
		flex-wrap: wrap;
	}

	.capabilities li {
		background: rgba(15, 123, 108, 0.15);
		border-radius: 4px;
		padding: 0.1rem 0.45rem;
		font-size: 0.75rem;
		font-weight: 600;
	}

	.modal-actions {
		display: flex;
		justify-content: flex-end;
		gap: 0.6rem;
		padding-top: 0.5rem;
	}

	.btn-primary {
		padding: 0.5rem 1.2rem;
		background: var(--accent, #0f7b6c);
		color: #fff;
		border: none;
		border-radius: 6px;
		font-size: 0.9rem;
		font-weight: 600;
		cursor: pointer;
		transition: opacity 0.15s;
	}

	.btn-primary:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.btn-secondary {
		padding: 0.5rem 1.2rem;
		background: transparent;
		color: var(--accent, #0f7b6c);
		border: 1px solid var(--accent, #0f7b6c);
		border-radius: 6px;
		font-size: 0.9rem;
		font-weight: 600;
		cursor: pointer;
		transition:
			background 0.15s,
			opacity 0.15s;
	}

	.btn-secondary:hover {
		background: rgba(15, 123, 108, 0.08);
	}

	.btn-secondary:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.btn-cancel {
		padding: 0.5rem 1.2rem;
		background: transparent;
		color: var(--text-secondary, #666);
		border: 1px solid var(--border-color, rgba(15, 26, 31, 0.2));
		border-radius: 6px;
		font-size: 0.9rem;
		cursor: pointer;
		transition: background 0.15s;
	}

	.btn-cancel:hover {
		background: var(--hover-bg, rgba(15, 26, 31, 0.06));
	}
</style>
