<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import SettingsSection from '$lib/components/settings/SettingsSection.svelte';
	import EmptyState from '$lib/components/settings/EmptyState.svelte';
	import LoadingSpinner from '$lib/components/settings/LoadingSpinner.svelte';
	import ErrorMessage from '$lib/components/settings/ErrorMessage.svelte';
	import ConfirmDialog from '$lib/components/settings/ConfirmDialog.svelte';
	import SaveStatusBanner from '$lib/components/settings/SaveStatusBanner.svelte';
	import type { SaveStatus } from '$lib/components/settings/SaveStatusBanner.svelte';
	import {
		getDownloadClients,
		createDownloadClient,
		updateDownloadClient,
		deleteDownloadClient,
		ApiError
	} from '$lib/api';
	import type {
		DownloadClient,
		CreateDownloadClientRequest,
		UpdateDownloadClientRequest
	} from '$lib/types';

	// ── state ──────────────────────────────────────────────────────────────────
	let clients = $state<DownloadClient[]>([]);
	let loading = $state(true);
	let loadError = $state('');
	let saveStatus = $state<SaveStatus>('idle');
	let saveError = $state('');

	// modal state
	let modalOpen = $state(false);
	let editingClient = $state<DownloadClient | null>(null);

	// form fields
	let formName = $state('');
	let formClientType = $state('qbittorrent');
	let formBaseUrl = $state('');
	let formUsername = $state('');
	let formPassword = $state('');
	let formCategory = $state('');
	let formEnabled = $state(true);
	let formErrors = $state<Record<string, string>>({});
	let formSaving = $state(false);

	// delete state
	let deleteTarget = $state<DownloadClient | null>(null);
	let deleteDialogOpen = $state(false);
	let deleting = $state(false);

	// banner auto-clear timer
	let saveStatusTimer: ReturnType<typeof setTimeout> | null = null;

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
	const CLIENT_TYPES = [
		{ value: 'qbittorrent', label: 'qBittorrent' },
		{ value: 'transmission', label: 'Transmission' },
		{ value: 'deluge', label: 'Deluge' },
		{ value: 'sabnzbd', label: 'SABnzbd' },
		{ value: 'nzbget', label: 'NZBGet' }
	];

	// ── data loading ───────────────────────────────────────────────────────────
	async function load() {
		loading = true;
		loadError = '';
		try {
			const result = await getDownloadClients({ limit: 100 });
			clients = result.items;
		} catch (err) {
			loadError = err instanceof ApiError ? err.message : 'Failed to load download clients.';
		} finally {
			loading = false;
		}
	}

	onMount(load);

	// ── modal helpers ──────────────────────────────────────────────────────────
	function openAdd() {
		editingClient = null;
		formName = '';
		formClientType = 'qbittorrent';
		formBaseUrl = '';
		formUsername = '';
		formPassword = '';
		formCategory = '';
		formEnabled = true;
		formErrors = {};
		modalOpen = true;
	}

	function openEdit(client: DownloadClient) {
		editingClient = client;
		formName = client.name;
		formClientType = client.client_type;
		formBaseUrl = client.base_url;
		formUsername = client.username ?? '';
		formPassword = '';
		formCategory = client.category ?? '';
		formEnabled = client.enabled;
		formErrors = {};
		modalOpen = true;
	}

	function closeModal() {
		modalOpen = false;
		editingClient = null;
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
				errors.base_url = 'Must be a valid URL (e.g. http://localhost:8080).';
			}
		}
		formErrors = errors;
		return Object.keys(errors).length === 0;
	}

	async function saveForm() {
		if (!validateForm()) return;
		formSaving = true;
		saveStatus = 'saving';
		saveError = '';
		try {
			if (editingClient) {
				const payload: UpdateDownloadClientRequest = {
					name: formName.trim(),
					client_type: formClientType,
					base_url: formBaseUrl.trim(),
					username: formUsername.trim(),
					password: formPassword || undefined,
					category: formCategory.trim(),
					enabled: formEnabled
				};
				const updated = await updateDownloadClient(editingClient.id, payload);
				clients = clients.map((c) => (c.id === updated.id ? updated : c));
			} else {
				const payload: CreateDownloadClientRequest = {
					name: formName.trim(),
					client_type: formClientType,
					base_url: formBaseUrl.trim(),
					username: formUsername.trim() || null,
					password: formPassword || null,
					category: formCategory.trim() || null,
					enabled: formEnabled
				};
				const created = await createDownloadClient(payload);
				clients = [...clients, created];
			}
			closeModal();
			saveStatus = 'saved';
			scheduleBannerClear();
		} catch (err) {
			saveError = err instanceof ApiError ? err.message : 'Save failed.';
			saveStatus = 'error';
		} finally {
			formSaving = false;
		}
	}

	// ── toggle enabled ─────────────────────────────────────────────────────────
	async function toggleEnabled(client: DownloadClient) {
		try {
			const updated = await updateDownloadClient(client.id, { enabled: !client.enabled });
			clients = clients.map((c) => (c.id === updated.id ? updated : c));
		} catch (err) {
			saveError = err instanceof ApiError ? err.message : 'Failed to update client.';
			saveStatus = 'error';
		}
	}

	// ── delete ─────────────────────────────────────────────────────────────────
	function openDelete(client: DownloadClient) {
		deleteTarget = client;
		deleteDialogOpen = true;
	}

	async function confirmDelete() {
		if (!deleteTarget) return;
		deleting = true;
		try {
			await deleteDownloadClient(deleteTarget.id);
			clients = clients.filter((c) => c.id !== deleteTarget!.id);
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

	function clientTypeLabel(type: string): string {
		return CLIENT_TYPES.find((t) => t.value === type)?.label ?? type;
	}
</script>

<SettingsSection
	title="Download Clients"
	description="Configure download clients (qBittorrent, Transmission, Deluge, SABnzbd, NZBGet) used to fetch releases."
>
	{#snippet actions()}
		<SaveStatusBanner status={saveStatus} errorMessage={saveError} />
		<button class="btn-primary" onclick={openAdd}>Add Client</button>
	{/snippet}

	{#if loading}
		<LoadingSpinner label="Loading download clients…" />
	{:else if loadError}
		<ErrorMessage message={loadError} onretry={load} />
	{:else if clients.length === 0}
		<EmptyState
			message="No download clients configured."
			actionLabel="Add Client"
			onaction={openAdd}
		/>
	{:else}
		<ul class="client-list" role="list">
			{#each clients as client (client.id)}
				<li class="client-item" class:disabled={!client.enabled}>
					<div class="client-info">
						<span class="client-name">{client.name}</span>
						<span class="client-meta">
							{clientTypeLabel(client.client_type)} · {client.base_url}
							{#if client.username} · {client.username}{/if}
						</span>
					</div>
					<div class="client-actions">
						<label class="toggle" title={client.enabled ? 'Disable' : 'Enable'}>
							<input
								type="checkbox"
								checked={client.enabled}
								onchange={() => toggleEnabled(client)}
								aria-label="Enable {client.name}"
							/>
							<span class="toggle-track"></span>
						</label>
						<button class="btn-icon" onclick={() => openEdit(client)} aria-label="Edit {client.name}">
							Edit
						</button>
						<button
							class="btn-icon destructive"
							onclick={() => openDelete(client)}
							aria-label="Delete {client.name}"
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
				{editingClient ? 'Edit Download Client' : 'Add Download Client'}
			</h3>

			<form
				class="modal-form"
				onsubmit={(e) => {
					e.preventDefault();
					saveForm();
				}}
			>
				<div class="field" class:has-error={!!formErrors.name}>
					<label for="dc-name">Name <span aria-hidden="true">*</span></label>
					<input id="dc-name" type="text" bind:value={formName} placeholder="My qBittorrent" />
					{#if formErrors.name}<span class="field-error">{formErrors.name}</span>{/if}
				</div>

				<div class="field">
					<label for="dc-type">Client Type</label>
					<select id="dc-type" bind:value={formClientType}>
						{#each CLIENT_TYPES as type}
							<option value={type.value}>{type.label}</option>
						{/each}
					</select>
				</div>

				<div class="field" class:has-error={!!formErrors.base_url}>
					<label for="dc-url">Base URL <span aria-hidden="true">*</span></label>
					<input
						id="dc-url"
						type="url"
						bind:value={formBaseUrl}
						placeholder="http://localhost:8080"
					/>
					{#if formErrors.base_url}<span class="field-error">{formErrors.base_url}</span>{/if}
				</div>

				<div class="field">
					<label for="dc-username">Username</label>
					<input id="dc-username" type="text" bind:value={formUsername} autocomplete="username" />
				</div>

				<div class="field">
					<label for="dc-password">
						Password
						{#if editingClient && editingClient.has_password}
							<span class="hint">(leave blank to keep existing)</span>
						{/if}
					</label>
					<input
						id="dc-password"
						type="password"
						bind:value={formPassword}
						autocomplete="current-password"
					/>
				</div>

				<div class="field">
					<label for="dc-category">Category</label>
					<input id="dc-category" type="text" bind:value={formCategory} placeholder="chorrosion" />
				</div>

				<div class="field field-inline">
					<label for="dc-enabled">Enabled</label>
					<input id="dc-enabled" type="checkbox" bind:checked={formEnabled} />
				</div>

				<div class="modal-actions">
					<button type="button" class="btn-cancel" onclick={closeModal}>Cancel</button>
					<button type="submit" class="btn-primary" disabled={formSaving}>
						{formSaving ? 'Saving…' : editingClient ? 'Save Changes' : 'Add Client'}
					</button>
				</div>
			</form>
		</div>
	</div>
{/if}

<!-- ── Delete confirmation ──────────────────────────────────────────────── -->
<ConfirmDialog
	open={deleteDialogOpen}
	title="Delete Download Client"
	message={`Delete "${deleteTarget?.name ?? ''}"? This cannot be undone.`}
	confirmLabel={deleting ? 'Deleting…' : 'Delete'}
	destructive
	onconfirm={confirmDelete}
	oncancel={cancelDelete}
/>

<style>
	.client-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.client-item {
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

	.client-item:hover {
		border-color: var(--accent, #0f7b6c);
	}

	.client-item.disabled {
		opacity: 0.6;
	}

	.client-info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
	}

	.client-name {
		font-weight: 600;
		font-size: 0.95rem;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.client-meta {
		font-size: 0.8rem;
		color: var(--text-secondary, #666);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.client-actions {
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
		font-weight: 500;
		cursor: pointer;
		color: var(--text-primary, #0f1a1f);
		transition:
			background 0.12s,
			border-color 0.12s;
	}

	.btn-icon:hover {
		background: rgba(0, 0, 0, 0.04);
	}

	.btn-icon.destructive {
		color: var(--error, #b6422e);
		border-color: transparent;
	}

	.btn-icon.destructive:hover {
		background: rgba(182, 66, 46, 0.08);
		border-color: rgba(182, 66, 46, 0.3);
	}

	/* Primary button */
	.btn-primary {
		padding: 0.55rem 1.2rem;
		background: var(--accent, #0f7b6c);
		color: #fff;
		border: none;
		border-radius: 6px;
		font-size: 0.875rem;
		font-weight: 600;
		cursor: pointer;
		transition: opacity 0.12s;
		white-space: nowrap;
	}

	.btn-primary:hover:not(:disabled) {
		opacity: 0.88;
	}

	.btn-primary:disabled {
		opacity: 0.55;
		cursor: not-allowed;
	}

	.btn-cancel {
		padding: 0.55rem 1.1rem;
		background: transparent;
		border: 1px solid var(--border-color, rgba(15, 26, 31, 0.2));
		border-radius: 6px;
		font-size: 0.875rem;
		font-weight: 500;
		cursor: pointer;
		color: var(--text-primary, #0f1a1f);
		transition: background 0.12s;
	}

	.btn-cancel:hover {
		background: rgba(0, 0, 0, 0.04);
	}

	/* Modal */
	.modal-backdrop {
		position: fixed;
		inset: 0;
		z-index: 500;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.modal-scrim {
		position: absolute;
		inset: 0;
		background: rgba(15, 26, 31, 0.45);
		backdrop-filter: blur(2px);
	}

	.modal-panel {
		position: relative;
		z-index: 1;
		background: var(--paper, #fffdf7);
		border: 1px solid var(--border-color, rgba(15, 26, 31, 0.15));
		border-radius: 12px;
		padding: 1.75rem 2rem;
		width: min(480px, calc(100vw - 2rem));
		max-height: calc(100dvh - 4rem);
		overflow-y: auto;
		box-shadow: 0 16px 48px rgba(15, 26, 31, 0.18);
		animation: pop-in 0.15s ease-out;
	}

	.modal-title {
		margin: 0 0 1.5rem 0;
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

	.field-inline {
		flex-direction: row;
		align-items: center;
		gap: 0.75rem;
	}

	.field label {
		font-size: 0.875rem;
		font-weight: 500;
		color: var(--text-primary, #0f1a1f);
	}

	.field input[type='text'],
	.field input[type='url'],
	.field input[type='password'],
	.field select {
		padding: 0.5rem 0.75rem;
		border: 1px solid var(--border-color, rgba(15, 26, 31, 0.2));
		border-radius: 6px;
		background: var(--bg-primary, #f2efe7);
		color: var(--text-primary, #0f1a1f);
		font-size: 0.875rem;
		transition: border-color 0.12s;
		width: 100%;
		box-sizing: border-box;
	}

	.field input:focus,
	.field select:focus {
		outline: none;
		border-color: var(--accent, #0f7b6c);
		box-shadow: 0 0 0 3px rgba(15, 123, 108, 0.12);
	}

	.has-error input {
		border-color: var(--error, #b6422e);
	}

	.field-error {
		font-size: 0.78rem;
		color: var(--error, #b6422e);
	}

	.hint {
		font-size: 0.78rem;
		font-weight: 400;
		color: var(--text-secondary, #666);
		margin-left: 0.25rem;
	}

	.modal-actions {
		display: flex;
		justify-content: flex-end;
		gap: 0.75rem;
		margin-top: 0.5rem;
		padding-top: 1rem;
		border-top: 1px solid var(--border-color, rgba(15, 26, 31, 0.1));
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

	@media (max-width: 600px) {
		.client-item {
			flex-direction: column;
			align-items: flex-start;
		}

		.client-actions {
			width: 100%;
			justify-content: flex-end;
		}

		.modal-panel {
			padding: 1.25rem;
		}
	}
</style>
