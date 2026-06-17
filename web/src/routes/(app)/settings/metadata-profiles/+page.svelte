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
		getMetadataProfiles,
		createMetadataProfile,
		updateMetadataProfile,
		deleteMetadataProfile,
		bulkMetadataProfiles,
		exportMetadataProfiles,
		importMetadataProfiles,
		ApiError
	} from '$lib/api';
	import { useUnsavedGuard } from '$lib/stores/unsavedGuard';
	import { classifyFormError, mapClassifiedSaveErrorToUiState } from '$lib/settingsValidation';
	import type {
		MetadataProfile,
		CreateMetadataProfileRequest,
		SettingsImportConflictPolicy,
		SettingsImportResponse,
		UpdateMetadataProfileRequest
	} from '$lib/types';

	const PRIMARY_ALBUM_TYPE_PRESETS = [
		'Album',
		'Single',
		'EP',
		'Broadcast',
		'Other'
	];

	const SECONDARY_ALBUM_TYPE_PRESETS = [
		'Compilation',
		'Soundtrack',
		'Live',
		'Remix',
		'Demo',
		'DJ-mix',
		'MixTape/Street',
		'Interview',
		'Audio drama',
		'Audiobook'
	];

	const RELEASE_STATUS_PRESETS = ['Official', 'Promotion', 'Bootleg', 'Pseudo-release'];

	function uniqueSorted(values: Iterable<string>): string[] {
		return [...new Set(values)].filter((v) => v.trim().length > 0).sort((a, b) => a.localeCompare(b));
	}

	function orderedValues(selectedValues: Iterable<string>, presets: string[]): string[] {
		const selected = new Set(selectedValues);
		const presetSet = new Set(presets);
		const presetOrdered = presets.filter((v) => selected.has(v));
		const customOrdered = uniqueSorted(selected).filter((v) => !presetSet.has(v));
		return [...presetOrdered, ...customOrdered];
	}

	function allOptions(selectedValues: Iterable<string>, presets: string[]): string[] {
		const presetSet = new Set(presets);
		const custom = uniqueSorted(selectedValues).filter((v) => !presetSet.has(v));
		return [...presets, ...custom];
	}

	let profiles = $state<MetadataProfile[]>([]);
	let selectedIds = $state<Set<string>>(new Set());
	let loading = $state(true);
	let loadError = $state('');
	let saveStatus = $state<SaveStatus>('idle');
	let saveError = $state('');
	let formDirty = $state(false);
	let leaveDialogOpen = $state(false);
	let initialFormSnapshot = '';

	let modalOpen = $state(false);
	let editingProfile = $state<MetadataProfile | null>(null);
	let formSaving = $state(false);

	let formName = $state('');
	let formPrimaryTypes = $state<Set<string>>(new Set());
	let formSecondaryTypes = $state<Set<string>>(new Set());
	let formReleaseStatuses = $state<Set<string>>(new Set());
	let formCustomPrimaryType = $state('');
	let formCustomSecondaryType = $state('');
	let formCustomReleaseStatus = $state('');
	let formErrors = $state<Record<string, string>>({});

	let deleteDialogOpen = $state(false);
	let deleteTarget = $state<MetadataProfile | null>(null);
	let deleting = $state(false);
	let bulkDeleteDialogOpen = $state(false);
	let bulkActing = $state(false);
	let importDialogOpen = $state(false);
	let importApplying = $state(false);
	let importPolicy = $state<SettingsImportConflictPolicy>('merge');
	let importVersion = $state('1');
	let importItems = $state<CreateMetadataProfileRequest[]>([]);
	let importPreview = $state<SettingsImportResponse | null>(null);
	let importFileInput: HTMLInputElement | null = null;

	let saveStatusTimer: ReturnType<typeof setTimeout> | null = null;
	const unsavedGuard = useUnsavedGuard(() => {
		leaveDialogOpen = true;
	});

	function getFormSnapshot(): string {
		return JSON.stringify({
			name: formName.trim(),
			primaryAlbumTypes: orderedValues(formPrimaryTypes, PRIMARY_ALBUM_TYPE_PRESETS),
			secondaryAlbumTypes: orderedValues(formSecondaryTypes, SECONDARY_ALBUM_TYPE_PRESETS),
			releaseStatuses: orderedValues(formReleaseStatuses, RELEASE_STATUS_PRESETS)
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

	let primaryTypeOptions = $derived(
		allOptions(
			[...formPrimaryTypes, ...(editingProfile?.primary_album_types ?? [])],
			PRIMARY_ALBUM_TYPE_PRESETS
		)
	);
	let secondaryTypeOptions = $derived(
		allOptions(
			[...formSecondaryTypes, ...(editingProfile?.secondary_album_types ?? [])],
			SECONDARY_ALBUM_TYPE_PRESETS
		)
	);
	let releaseStatusOptions = $derived(
		allOptions(
			[...formReleaseStatuses, ...(editingProfile?.release_statuses ?? [])],
			RELEASE_STATUS_PRESETS
		)
	);

	function scheduleBannerClear() {
		if (saveStatusTimer !== null) clearTimeout(saveStatusTimer);
		saveStatusTimer = setTimeout(() => {
			saveStatus = 'idle';
			saveStatusTimer = null;
		}, 2500);
	}

	async function exportItems() {
		try {
			const payload = await exportMetadataProfiles();
			const blob = new Blob([JSON.stringify(payload, null, 2)], { type: 'application/json' });
			const url = URL.createObjectURL(blob);
			const link = document.createElement('a');
			link.href = url;
			link.download = 'metadata-profiles.export.json';
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
		importPreview = await importMetadataProfiles(
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
			const result = await importMetadataProfiles(
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

	onDestroy(() => {
		if (saveStatusTimer !== null) clearTimeout(saveStatusTimer);
	});

	async function load() {
		loading = true;
		loadError = '';
		try {
			const result = await getMetadataProfiles({ limit: 100 });
			profiles = result.items;
		} catch (err) {
			loadError = err instanceof ApiError ? err.message : 'Failed to load metadata profiles.';
		} finally {
			loading = false;
		}
	}

	onMount(load);

	let hasSelection = $derived(selectedIds.size > 0);
	let allSelected = $derived(profiles.length > 0 && profiles.every((profile) => selectedIds.has(profile.id)));

	function openAdd() {
		editingProfile = null;
		formName = '';
		formPrimaryTypes = new Set(['Album']);
		formSecondaryTypes = new Set();
		formReleaseStatuses = new Set(['Official']);
		formCustomPrimaryType = '';
		formCustomSecondaryType = '';
		formCustomReleaseStatus = '';
		formErrors = {};
		leaveDialogOpen = false;
		modalOpen = true;
		initialFormSnapshot = getFormSnapshot();
		syncDirtyState();
	}

	function openEdit(profile: MetadataProfile) {
		editingProfile = profile;
		formName = profile.name;
		formPrimaryTypes = new Set(profile.primary_album_types);
		formSecondaryTypes = new Set(profile.secondary_album_types);
		formReleaseStatuses = new Set(profile.release_statuses);
		formCustomPrimaryType = '';
		formCustomSecondaryType = '';
		formCustomReleaseStatus = '';
		formErrors = {};
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
		editingProfile = null;
	}

	function toggleSelection(setterTarget: Set<string>, value: string): Set<string> {
		const next = new Set(setterTarget);
		if (next.has(value)) {
			next.delete(value);
		} else {
			next.add(value);
		}
		return next;
	}

	function addCustomValue(
		value: string,
		currentSet: Set<string>,
		setSet: (s: Set<string>) => void,
		setField: (v: string) => void
	) {
		const trimmed = value.trim();
		if (!trimmed) return;
		setSet(new Set([...currentSet, trimmed]));
		setField('');
		syncDirtyState();
	}

	function validateForm(): boolean {
		const errors: Record<string, string> = {};
		if (!formName.trim()) errors.name = 'Name is required.';
		formErrors = errors;
		return Object.keys(errors).length === 0;
	}

	async function saveForm() {
		if (!validateForm()) return;
		formSaving = true;
		saveStatus = 'saving';
		saveError = '';

		const primaryTypes = orderedValues(formPrimaryTypes, PRIMARY_ALBUM_TYPE_PRESETS);
		const secondaryTypes = orderedValues(formSecondaryTypes, SECONDARY_ALBUM_TYPE_PRESETS);
		const releaseStatuses = orderedValues(formReleaseStatuses, RELEASE_STATUS_PRESETS);

		try {
			if (editingProfile) {
				const payload: UpdateMetadataProfileRequest = {
					name: formName.trim(),
					primary_album_types: primaryTypes,
					secondary_album_types: secondaryTypes,
					release_statuses: releaseStatuses
				};
				const updated = await updateMetadataProfile(editingProfile.id, payload);
				profiles = profiles.map((profile) => (profile.id === updated.id ? updated : profile));
			} else {
				const payload: CreateMetadataProfileRequest = {
					name: formName.trim(),
					primary_album_types: primaryTypes,
					secondary_album_types: secondaryTypes,
					release_statuses: releaseStatuses
				};
				const created = await createMetadataProfile(payload);
				profiles = [...profiles, created];
			}

			unsavedGuard.markClean();
			formDirty = false;
			initialFormSnapshot = getFormSnapshot();
			closeModal();
			saveStatus = 'saved';
			scheduleBannerClear();
		} catch (err) {
			const classified = classifyFormError(err, [
				{ field: 'name', messages: ['name cannot be empty', 'already exists'] }
			]);
			const uiState = mapClassifiedSaveErrorToUiState(classified, formErrors);
			formErrors = uiState.formErrors;
			saveStatus = uiState.saveStatus;
			saveError = uiState.saveError;
		} finally {
			formSaving = false;
		}
	}

	function openDelete(profile: MetadataProfile) {
		deleteTarget = profile;
		deleteDialogOpen = true;
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
		selectedIds = new Set(profiles.map((profile) => profile.id));
	}

	async function runBulkAction(action: 'delete') {
		if (selectedIds.size === 0) return;

		const ids = [...selectedIds];
		const selectedSet = new Set(ids);
		const originalProfiles = profiles;

		if (action === 'delete') {
			profiles = profiles.filter((profile) => !selectedSet.has(profile.id));
		}

		saveStatus = 'saving';
		saveError = '';
		bulkActing = true;

		try {
			const result = await bulkMetadataProfiles({ action, ids });
			const failed = result.results.filter((item) => !item.success);
			if (failed.length > 0) {
				const failedSet = new Set(failed.map((item) => item.id));
				if (action === 'delete') {
					profiles = originalProfiles.filter(
						(profile) => !selectedSet.has(profile.id) || failedSet.has(profile.id)
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
			profiles = originalProfiles;
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

	function cancelDelete() {
		deleteDialogOpen = false;
		deleteTarget = null;
	}

	async function confirmDelete() {
		if (!deleteTarget) return;
		deleting = true;
		try {
			await deleteMetadataProfile(deleteTarget.id);
			profiles = profiles.filter((profile) => profile.id !== deleteTarget!.id);
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

	function confirmLeave() {
		leaveDialogOpen = false;
		modalOpen = false;
		editingProfile = null;
		void unsavedGuard.confirmNavigation();
	}

	function cancelLeave() {
		leaveDialogOpen = false;
		unsavedGuard.discardNavigation();
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
	title="Metadata Profiles"
	description="Control which metadata sources and fields are used when tagging music files."
>
	{#snippet actions()}
		<SaveStatusBanner status={saveStatus} errorMessage={saveError} />
		<button class="btn-secondary" onclick={exportItems}>Export</button>
		<button class="btn-secondary" onclick={triggerImport}>Import</button>
		<input bind:this={importFileInput} type="file" accept="application/json" hidden onchange={handleImportFile} />
		{#if hasSelection}
			<div class="bulk-toolbar">
				<button class="btn-icon destructive" onclick={openBulkDelete} disabled={bulkActing}>
					Delete Selected
				</button>
			</div>
		{/if}
		<button class="btn-primary" onclick={openAdd}>Add Profile</button>
	{/snippet}

	{#if loading}
		<LoadingSpinner label="Loading metadata profiles..." />
	{:else if loadError}
		<ErrorMessage message={loadError} onretry={load} />
	{:else if profiles.length === 0}
		<EmptyState
			message="No metadata profiles configured."
			actionLabel="Add Profile"
			onaction={openAdd}
		/>
	{:else}
		<div class="bulk-select-row">
			<label>
				<input type="checkbox" checked={allSelected} onchange={toggleSelectAll} />
				Select All ({selectedIds.size}/{profiles.length})
			</label>
		</div>
		<ul class="profile-list" role="list">
			{#each profiles as profile (profile.id)}
				<li class="profile-item">
					<input
						type="checkbox"
						checked={selectedIds.has(profile.id)}
						onchange={() => toggleRowSelection(profile.id)}
						aria-label="Select {profile.name}"
					/>
					<div class="profile-info">
						<span class="profile-name">{profile.name}</span>
						<span class="profile-meta">
							Primary: {profile.primary_album_types.join(', ') || 'None'}
							| Secondary: {profile.secondary_album_types.join(', ') || 'None'}
							| Statuses: {profile.release_statuses.join(', ') || 'None'}
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

{#if modalOpen}
	<div class="modal-backdrop" role="dialog" aria-modal="true" aria-labelledby="metadata-modal-title">
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<div class="modal-scrim" role="presentation" onclick={closeModal}></div>
		<div class="modal-panel">
			<h3 class="modal-title" id="metadata-modal-title">
				{editingProfile ? 'Edit Metadata Profile' : 'Add Metadata Profile'}
			</h3>

			<form
				class="modal-form"
				onsubmit={(e) => {
					e.preventDefault();
					saveForm();
				}}
			>
				<div class="field" class:has-error={!!formErrors.name}>
					<label for="metadata-name">Name <span aria-hidden="true">*</span></label>
					<input
						id="metadata-name"
						type="text"
						bind:value={formName}
						placeholder="MusicBrainz Default"
						oninput={() => {
							clearFieldError('name');
							syncDirtyState();
						}}
					/>
					{#if formErrors.name}<span class="field-error">{formErrors.name}</span>{/if}
				</div>

				<div class="field-group">
					<span class="field-group-title">Primary Album Types</span>
					<span class="field-hint">Choose the primary release types to keep when metadata is imported or refreshed.</span>
					<div class="option-grid">
						{#each primaryTypeOptions as value}
							<label class="option-check">
								<input
									type="checkbox"
									checked={formPrimaryTypes.has(value)}
									onchange={() => {
									formPrimaryTypes = toggleSelection(formPrimaryTypes, value);
									clearFieldError('primary_album_types');
									syncDirtyState();
								}}
								/>
								{value}
							</label>
						{/each}
					</div>
					<div class="custom-row">
						<input
							type="text"
							bind:value={formCustomPrimaryType}
							placeholder="Custom primary type"
							oninput={() => {
								clearFieldError('primary_album_types');
								syncDirtyState();
							}}
							onkeydown={(e) => {
								if (e.key === 'Enter') {
									e.preventDefault();
									addCustomValue(
										formCustomPrimaryType,
										formPrimaryTypes,
										(next) => (formPrimaryTypes = next),
										(next) => (formCustomPrimaryType = next)
									);
								}
							}}
						/>
						<button
							type="button"
							class="btn-add-custom"
							onclick={() =>
								addCustomValue(
									formCustomPrimaryType,
									formPrimaryTypes,
									(next) => (formPrimaryTypes = next),
									(next) => (formCustomPrimaryType = next)
								)}
						>
							Add
						</button>
					</div>
				</div>

				<div class="field-group">
					<span class="field-group-title">Secondary Album Types</span>
					<span class="field-hint">Use secondary types to include or exclude compilations, live releases, soundtracks, and similar variants.</span>
					<div class="option-grid">
						{#each secondaryTypeOptions as value}
							<label class="option-check">
								<input
									type="checkbox"
									checked={formSecondaryTypes.has(value)}
									onchange={() => {
									formSecondaryTypes = toggleSelection(formSecondaryTypes, value);
									clearFieldError('secondary_album_types');
									syncDirtyState();
								}}
								/>
								{value}
							</label>
						{/each}
					</div>
					<div class="custom-row">
						<input
							type="text"
							bind:value={formCustomSecondaryType}
							placeholder="Custom secondary type"
							oninput={() => {
								clearFieldError('secondary_album_types');
								syncDirtyState();
							}}
							onkeydown={(e) => {
								if (e.key === 'Enter') {
									e.preventDefault();
									addCustomValue(
										formCustomSecondaryType,
										formSecondaryTypes,
										(next) => (formSecondaryTypes = next),
										(next) => (formCustomSecondaryType = next)
									);
								}
							}}
						/>
						<button
							type="button"
							class="btn-add-custom"
							onclick={() =>
								addCustomValue(
									formCustomSecondaryType,
									formSecondaryTypes,
									(next) => (formSecondaryTypes = next),
									(next) => (formCustomSecondaryType = next)
								)}
						>
							Add
						</button>
					</div>
				</div>

				<div class="field-group">
					<span class="field-group-title">Release Statuses</span>
					<span class="field-hint">Statuses help filter official releases from promos, bootlegs, or pseudo-releases.</span>
					<div class="option-grid">
						{#each releaseStatusOptions as value}
							<label class="option-check">
								<input
									type="checkbox"
									checked={formReleaseStatuses.has(value)}
									onchange={() => {
									formReleaseStatuses = toggleSelection(formReleaseStatuses, value);
									clearFieldError('release_statuses');
									syncDirtyState();
								}}
								/>
								{value}
							</label>
						{/each}
					</div>
					<div class="custom-row">
						<input
							type="text"
							bind:value={formCustomReleaseStatus}
							placeholder="Custom release status"
							oninput={() => {
								clearFieldError('release_statuses');
								syncDirtyState();
							}}
							onkeydown={(e) => {
								if (e.key === 'Enter') {
									e.preventDefault();
									addCustomValue(
										formCustomReleaseStatus,
										formReleaseStatuses,
										(next) => (formReleaseStatuses = next),
										(next) => (formCustomReleaseStatus = next)
									);
								}
							}}
						/>
						<button
							type="button"
							class="btn-add-custom"
							onclick={() =>
								addCustomValue(
									formCustomReleaseStatus,
									formReleaseStatuses,
									(next) => (formReleaseStatuses = next),
									(next) => (formCustomReleaseStatus = next)
								)}
						>
							Add
						</button>
					</div>
				</div>

				<div class="modal-actions">
					<button type="button" class="btn-cancel" onclick={closeModal}>Cancel</button>
					<button type="submit" class="btn-primary" disabled={formSaving || !formDirty}>
						{formSaving ? 'Saving...' : editingProfile ? 'Save Changes' : 'Add Profile'}
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

<ConfirmDialog
	open={deleteDialogOpen}
	title="Delete Metadata Profile"
	message={`Delete "${deleteTarget?.name ?? ''}"? This cannot be undone.`}
	confirmLabel={deleting ? 'Deleting...' : 'Delete'}
	destructive
	onconfirm={confirmDelete}
	oncancel={cancelDelete}
/>

<ConfirmDialog
	open={bulkDeleteDialogOpen}
	title="Delete Selected Metadata Profiles"
	message={`Delete ${selectedIds.size} selected metadata profile(s)? This cannot be undone.`}
	confirmLabel={bulkActing ? 'Deleting...' : 'Delete Selected'}
	destructive
	onconfirm={confirmBulkDelete}
	oncancel={() => (bulkDeleteDialogOpen = false)}
/>

<ImportPreviewDialog
	open={importDialogOpen}
	title="Preview Metadata Profile Import"
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
	.profile-list {
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
		width: min(640px, 95vw);
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

	.field input[type='text'] {
		padding: 0.5rem 0.75rem;
		border: 1px solid var(--border-color, rgba(15, 26, 31, 0.2));
		border-radius: 6px;
		font-size: 0.9rem;
		background: var(--surface, #fff);
		color: var(--text-primary, #0f1a1f);
		transition: border-color 0.15s;
	}

	.field input:focus {
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
		line-height: 1.4;
		color: var(--text-secondary, #666);
	}

	.field-group {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.field-group-title {
		font-size: 0.85rem;
		font-weight: 600;
		color: var(--text-primary, #0f1a1f);
	}

	.option-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
		gap: 0.4rem 0.75rem;
		padding: 0.6rem 0.75rem;
		border: 1px solid var(--border-color, rgba(15, 26, 31, 0.15));
		border-radius: 6px;
		background: var(--surface, #fff);
	}

	.option-check {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		font-size: 0.85rem;
		cursor: pointer;
		user-select: none;
	}

	.option-check input[type='checkbox'] {
		width: 1rem;
		height: 1rem;
		accent-color: var(--accent, #0f7b6c);
		flex-shrink: 0;
	}

	.custom-row {
		display: flex;
		gap: 0.5rem;
	}

	.custom-row input {
		flex: 1;
		padding: 0.4rem 0.65rem;
		border: 1px solid var(--border-color, rgba(15, 26, 31, 0.2));
		border-radius: 6px;
		font-size: 0.85rem;
		background: var(--surface, #fff);
		color: var(--text-primary, #0f1a1f);
	}

	.custom-row input:focus {
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
