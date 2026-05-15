<script lang="ts">
	import type {
		SettingsImportConflictPolicy,
		SettingsImportPreviewItem,
		SettingsImportSummary
	} from '$lib/types';

	interface Props {
		open: boolean;
		title: string;
		summary: SettingsImportSummary;
		preview: SettingsImportPreviewItem[];
		policy: SettingsImportConflictPolicy;
		applying?: boolean;
		onPolicyChange: (policy: SettingsImportConflictPolicy) => void;
		onConfirm: () => void;
		onCancel: () => void;
	}

	let {
		open,
		title,
		summary,
		preview,
		policy,
		applying = false,
		onPolicyChange,
		onConfirm,
		onCancel
	}: Props = $props();
</script>

{#if open}
	<div class="dialog-backdrop" role="dialog" aria-modal="true" aria-label={title}>
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<div class="dialog-scrim" role="presentation" onclick={onCancel}></div>
		<div class="dialog-panel">
			<h3>{title}</h3>
			<div class="summary-row">
				<span>Added: {summary.added}</span>
				<span>Updated: {summary.updated}</span>
				<span>Deleted: {summary.deleted}</span>
			</div>
			<label class="policy-label" for="import-policy">Conflict policy</label>
			<select
				id="import-policy"
				value={policy}
				onchange={(event) =>
					onPolicyChange((event.currentTarget as HTMLSelectElement).value as SettingsImportConflictPolicy)}
			>
				<option value="merge">Merge</option>
				<option value="replace_all">Replace All</option>
			</select>
			<div class="preview-list" role="list">
				{#each preview.slice(0, 20) as item}
					<div class="preview-item" role="listitem">
						<span class="preview-action">{item.action}</span>
						<span>{item.name}</span>
					</div>
				{/each}
				{#if preview.length > 20}
					<div class="preview-more">+ {preview.length - 20} more</div>
				{/if}
			</div>
			<div class="dialog-actions">
				<button type="button" class="btn-cancel" onclick={onCancel}>Cancel</button>
				<button type="button" class="btn-confirm" onclick={onConfirm} disabled={applying}>
					{applying ? 'Importing…' : 'Apply Import'}
				</button>
			</div>
		</div>
	</div>
{/if}

<style>
	.dialog-backdrop {
		position: fixed;
		inset: 0;
		z-index: 1000;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.dialog-scrim {
		position: absolute;
		inset: 0;
		background: rgba(15, 26, 31, 0.45);
	}
	.dialog-panel {
		position: relative;
		z-index: 1;
		background: var(--paper, #fffdf7);
		border: 1px solid var(--border-color, rgba(15, 26, 31, 0.15));
		border-radius: 12px;
		padding: 1.25rem;
		max-width: 520px;
		width: calc(100% - 2rem);
	}
	.summary-row {
		display: flex;
		gap: 0.75rem;
		margin-bottom: 0.75rem;
		font-size: 0.85rem;
	}
	.policy-label {
		display: block;
		font-size: 0.8rem;
		margin-bottom: 0.25rem;
	}
	select {
		width: 100%;
		margin-bottom: 0.75rem;
	}
	.preview-list {
		max-height: 220px;
		overflow: auto;
		border: 1px solid var(--border-color, rgba(15, 26, 31, 0.2));
		border-radius: 6px;
		padding: 0.5rem;
		margin-bottom: 0.75rem;
	}
	.preview-item {
		display: flex;
		gap: 0.5rem;
		font-size: 0.8rem;
		padding: 0.15rem 0;
	}
	.preview-action {
		text-transform: uppercase;
		font-weight: 700;
		font-size: 0.7rem;
		color: var(--accent, #0f7b6c);
	}
	.preview-more {
		font-size: 0.75rem;
		color: var(--text-secondary, #666);
		padding-top: 0.25rem;
	}
	.dialog-actions {
		display: flex;
		justify-content: flex-end;
		gap: 0.5rem;
	}
	.btn-cancel,
	.btn-confirm {
		padding: 0.5rem 0.85rem;
		border-radius: 6px;
		cursor: pointer;
	}
	.btn-cancel {
		border: 1px solid var(--border-color, rgba(15, 26, 31, 0.2));
		background: transparent;
	}
	.btn-confirm {
		border: none;
		color: #fff;
		background: var(--accent, #0f7b6c);
	}
</style>
