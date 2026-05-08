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
		getQualityProfiles,
		createQualityProfile,
		updateQualityProfile,
		deleteQualityProfile,
		ApiError
	} from '$lib/api';
	import type {
		QualityProfile,
		CreateQualityProfileRequest,
		UpdateQualityProfileRequest
	} from '$lib/types';

	// ── constants ──────────────────────────────────────────────────────────────
	const PRESET_QUALITIES = [
		'FLAC',
		'MP3 320',
		'MP3 256',
		'MP3 192',
		'MP3 128',
		'AAC 256',
		'AAC 192',
		'OGG 320',
		'OGG 256',
		'Opus 320',
		'Opus 256'
	];
	const PRESET_QUALITY_SET = new Set(PRESET_QUALITIES);

	function sortedCustomQualities(values: Iterable<string>): string[] {
		return [...new Set(values)]
			.filter((q) => q.trim().length > 0 && !PRESET_QUALITY_SET.has(q))
			.sort((a, b) => a.localeCompare(b));
	}

	function orderedAllowedQualities(values: Iterable<string>): string[] {
		const selected = new Set(values);
		return [
			...PRESET_QUALITIES.filter((q) => selected.has(q)),
			...sortedCustomQualities(selected)
		];
	}

	// ── state ──────────────────────────────────────────────────────────────────
	let profiles = $state<QualityProfile[]>([]);
	let loading = $state(true);
	let loadError = $state('');
	let saveStatus = $state<SaveStatus>('idle');
	let saveError = $state('');

	// modal state
	let modalOpen = $state(false);
	let editingProfile = $state<QualityProfile | null>(null);

	// form fields
	let formName = $state('');
	let formAllowedQualities = $state<Set<string>>(new Set());
	let formCustomQuality = $state('');
	let formUpgradeAllowed = $state(true);
	let formCutoffQuality = $state('');
	let formErrors = $state<Record<string, string>>({});
	let formSaving = $state(false);

	// derived: sorted list of all qualities to show in the modal
	// (all presets + any custom ones in form state or the profile being edited)
	let allModalQualities = $derived((() => {
		const extras = editingProfile?.allowed_qualities ?? [];
		return [...PRESET_QUALITIES, ...sortedCustomQualities([...formAllowedQualities, ...extras])];
	})());
	let orderedFormAllowedQualities = $derived(orderedAllowedQualities(formAllowedQualities));

	// delete state
	let deleteTarget = $state<QualityProfile | null>(null);
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

	// ── data loading ───────────────────────────────────────────────────────────
	async function load() {
		loading = true;
		loadError = '';
		try {
			const result = await getQualityProfiles({ limit: 100 });
			profiles = result.items;
		} catch (err) {
			loadError = err instanceof ApiError ? err.message : 'Failed to load quality profiles.';
		} finally {
			loading = false;
		}
	}

	onMount(load);

	// ── modal helpers ──────────────────────────────────────────────────────────
	function openAdd() {
		editingProfile = null;
		formName = '';
		formAllowedQualities = new Set(['FLAC', 'MP3 320']);
		formCustomQuality = '';
		formUpgradeAllowed = true;
		formCutoffQuality = '';
		formErrors = {};
		modalOpen = true;
	}

	function openEdit(profile: QualityProfile) {
		editingProfile = profile;
		formName = profile.name;
		const allowedQualities = new Set(profile.allowed_qualities);
		formAllowedQualities = allowedQualities;
		formCustomQuality = '';
		formUpgradeAllowed = profile.upgrade_allowed;
		formCutoffQuality =
			profile.cutoff_quality && allowedQualities.has(profile.cutoff_quality)
				? profile.cutoff_quality
				: '';
		formErrors = {};
		modalOpen = true;
	}

	function closeModal() {
		modalOpen = false;
		editingProfile = null;
	}

	function toggleQuality(q: string) {
		const next = new Set(formAllowedQualities);
		if (next.has(q)) {
			next.delete(q);
			if (formCutoffQuality === q) formCutoffQuality = '';
		} else {
			next.add(q);
		}
		formAllowedQualities = next;
	}

	function addCustomQuality() {
		const q = formCustomQuality.trim();
		if (!q) return;
		formAllowedQualities = new Set([...formAllowedQualities, q]);
		formCustomQuality = '';
	}

	function validateForm(): boolean {
		const errors: Record<string, string> = {};
		if (!formName.trim()) errors.name = 'Name is required.';
		if (formAllowedQualities.size === 0) errors.qualities = 'Select at least one quality.';
		formErrors = errors;
		return Object.keys(errors).length === 0;
	}

	async function saveForm() {
		if (!validateForm()) return;
		formSaving = true;
		saveStatus = 'saving';
		saveError = '';
		const qualities = orderedAllowedQualities(formAllowedQualities);
		const cutoffQuality =
			formCutoffQuality && formAllowedQualities.has(formCutoffQuality) ? formCutoffQuality : '';
		try {
			if (editingProfile) {
				const payload: UpdateQualityProfileRequest = {
					name: formName.trim(),
					allowed_qualities: qualities,
					upgrade_allowed: formUpgradeAllowed,
					cutoff_quality: cutoffQuality || null
				};
				const updated = await updateQualityProfile(editingProfile.id, payload);
				profiles = profiles.map((p) => (p.id === updated.id ? updated : p));
			} else {
				const payload: CreateQualityProfileRequest = {
					name: formName.trim(),
					allowed_qualities: qualities,
					upgrade_allowed: formUpgradeAllowed,
					cutoff_quality: cutoffQuality || null
				};
				const created = await createQualityProfile(payload);
				profiles = [...profiles, created];
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

	// ── delete ─────────────────────────────────────────────────────────────────
	function openDelete(profile: QualityProfile) {
		deleteTarget = profile;
		deleteDialogOpen = true;
	}

	async function confirmDelete() {
		if (!deleteTarget) return;
		deleting = true;
		try {
			await deleteQualityProfile(deleteTarget.id);
			profiles = profiles.filter((p) => p.id !== deleteTarget!.id);
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
</script>

<svelte:window
	onkeydown={(e) => {
		if (e.key === 'Escape' && modalOpen) {
			e.preventDefault();
			e.stopPropagation();
			closeModal();
		}
	}}
/>

<SettingsSection
	title="Quality Profiles"
	description="Define which audio formats and bitrates are acceptable for automated downloading."
>
	{#snippet actions()}
		<SaveStatusBanner status={saveStatus} errorMessage={saveError} />
		<button class="btn-primary" onclick={openAdd}>Add Profile</button>
	{/snippet}

	{#if loading}
		<LoadingSpinner label="Loading quality profiles…" />
	{:else if loadError}
		<ErrorMessage message={loadError} onretry={load} />
	{:else if profiles.length === 0}
		<EmptyState
			message="No quality profiles configured."
			actionLabel="Add Profile"
			onaction={openAdd}
		/>
	{:else}
		<ul class="profile-list" role="list">
			{#each profiles as profile (profile.id)}
				<li class="profile-item">
					<div class="profile-info">
						<span class="profile-name">{profile.name}</span>
						<span class="profile-meta">
							{profile.allowed_qualities.join(', ')}
							{#if profile.cutoff_quality}&nbsp;· Cutoff: {profile.cutoff_quality}{/if}
							{#if profile.upgrade_allowed}&nbsp;· Upgrades on{/if}
						</span>
					</div>
					<div class="profile-actions">
						<button
							class="btn-icon"
							onclick={() => openEdit(profile)}
							aria-label="Edit {profile.name}"
						>
							Edit
						</button>
						<button
							class="btn-icon destructive"
							onclick={() => openDelete(profile)}
							aria-label="Delete {profile.name}"
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
{#if modalOpen}
	<div class="modal-backdrop" role="dialog" aria-modal="true" aria-labelledby="modal-title">
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<div class="modal-scrim" role="presentation" onclick={closeModal}></div>
		<div class="modal-panel">
			<h3 class="modal-title" id="modal-title">
				{editingProfile ? 'Edit Quality Profile' : 'Add Quality Profile'}
			</h3>

			<form
				class="modal-form"
				onsubmit={(e) => {
					e.preventDefault();
					saveForm();
				}}
			>
				<div class="field" class:has-error={!!formErrors.name}>
					<label for="qp-name">Name <span aria-hidden="true">*</span></label>
					<input id="qp-name" type="text" bind:value={formName} placeholder="My Quality Profile" />
					{#if formErrors.name}<span class="field-error">{formErrors.name}</span>{/if}
				</div>

				<div class="field" class:has-error={!!formErrors.qualities}>
					<span class="field-label">Allowed Qualities <span aria-hidden="true">*</span></span>
					<div class="quality-grid">
						{#each allModalQualities as q}
							<label class="quality-check">
								<input
									type="checkbox"
									checked={formAllowedQualities.has(q)}
									onchange={() => toggleQuality(q)}
								/>
								{q}
							</label>
						{/each}
					</div>
					<div class="custom-quality-row">
						<input
							type="text"
							bind:value={formCustomQuality}
							placeholder="Custom quality…"
							onkeydown={(e) => {
								if (e.key === 'Enter') {
									e.preventDefault();
									addCustomQuality();
								}
							}}
						/>
						<button type="button" class="btn-add-custom" onclick={addCustomQuality}>Add</button>
					</div>
					{#if formErrors.qualities}<span class="field-error">{formErrors.qualities}</span>{/if}
				</div>

				<div class="field">
					<label for="qp-cutoff">Cutoff Quality</label>
					<select id="qp-cutoff" bind:value={formCutoffQuality}>
						<option value="">— None —</option>
						{#each orderedFormAllowedQualities as q}
							<option value={q}>{q}</option>
						{/each}
					</select>
					<span class="field-hint">Stop upgrading once this quality is met.</span>
				</div>

				<div class="field field-inline">
					<label for="qp-upgrade">Upgrade Allowed</label>
					<input id="qp-upgrade" type="checkbox" bind:checked={formUpgradeAllowed} />
				</div>

				<div class="modal-actions">
					<button type="button" class="btn-cancel" onclick={closeModal}>Cancel</button>
					<button type="submit" class="btn-primary" disabled={formSaving}>
						{formSaving ? 'Saving…' : editingProfile ? 'Save Changes' : 'Add Profile'}
					</button>
				</div>
			</form>
		</div>
	</div>
{/if}

<!-- ── Delete confirmation ──────────────────────────────────────────────── -->
<ConfirmDialog
	open={deleteDialogOpen}
	title="Delete Quality Profile"
	message={`Delete "${deleteTarget?.name ?? ''}"? This cannot be undone.`}
	confirmLabel={deleting ? 'Deleting…' : 'Delete'}
	destructive
	onconfirm={confirmDelete}
	oncancel={cancelDelete}
/>

<style>
	.profile-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.profile-item {
		display: flex;
		align-items: center;
		gap: 1rem;
		padding: 0.85rem 1.1rem;
		background: var(--paper, #fffdf7);
		border: 1px solid var(--border-color, rgba(15, 26, 31, 0.12));
		border-radius: 8px;
		transition: border-color 0.15s;
	}

	.profile-item:hover {
		border-color: var(--accent, #0f7b6c);
	}

	.profile-info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
	}

	.profile-name {
		font-weight: 600;
		font-size: 0.95rem;
	}

	.profile-meta {
		font-size: 0.8rem;
		color: var(--text-secondary, #666);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.profile-actions {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex-shrink: 0;
	}

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
		width: min(520px, 95vw);
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

	.field label,
	.field-label {
		font-size: 0.85rem;
		font-weight: 600;
		color: var(--text-primary, #0f1a1f);
	}

	.field input[type='text'],
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
	}

	.field.has-error input {
		border-color: #c0392b;
	}

	.field-error {
		font-size: 0.78rem;
		color: #c0392b;
	}

	.field-hint {
		font-size: 0.78rem;
		color: var(--text-secondary, #666);
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

	.quality-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
		gap: 0.4rem 0.75rem;
		padding: 0.6rem 0.75rem;
		border: 1px solid var(--border-color, rgba(15, 26, 31, 0.15));
		border-radius: 6px;
		background: var(--surface, #fff);
	}

	.quality-check {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		font-size: 0.85rem;
		cursor: pointer;
		user-select: none;
	}

	.quality-check input[type='checkbox'] {
		width: 1rem;
		height: 1rem;
		accent-color: var(--accent, #0f7b6c);
		flex-shrink: 0;
	}

	.custom-quality-row {
		display: flex;
		gap: 0.5rem;
		margin-top: 0.4rem;
	}

	.custom-quality-row input {
		flex: 1;
		padding: 0.4rem 0.65rem;
		border: 1px solid var(--border-color, rgba(15, 26, 31, 0.2));
		border-radius: 6px;
		font-size: 0.85rem;
		background: var(--surface, #fff);
		color: var(--text-primary, #0f1a1f);
	}

	.custom-quality-row input:focus {
		outline: none;
		border-color: var(--accent, #0f7b6c);
	}

	.btn-add-custom {
		padding: 0.4rem 0.9rem;
		background: transparent;
		border: 1px solid var(--accent, #0f7b6c);
		border-radius: 6px;
		color: var(--accent, #0f7b6c);
		font-size: 0.85rem;
		font-weight: 600;
		cursor: pointer;
		transition: background 0.15s;
		flex-shrink: 0;
	}

	.btn-add-custom:hover {
		background: rgba(15, 123, 108, 0.08);
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
