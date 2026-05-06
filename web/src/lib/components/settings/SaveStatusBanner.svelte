<script lang="ts">
	export type SaveStatus = 'idle' | 'saving' | 'saved' | 'error';

	interface Props {
		status: SaveStatus;
		errorMessage?: string;
	}

	let { status, errorMessage }: Props = $props();
</script>

{#if status !== 'idle'}
	<div
		class="save-banner"
		class:saving={status === 'saving'}
		class:saved={status === 'saved'}
		class:error={status === 'error'}
		role={status === 'error' ? 'alert' : 'status'}
		aria-live="polite"
	>
		{#if status === 'saving'}
			<span class="spinner" aria-hidden="true"></span>
			Saving…
		{:else if status === 'saved'}
			<span class="icon" aria-hidden="true">✓</span>
			Saved
		{:else if status === 'error'}
			<span class="icon" aria-hidden="true">✕</span>
			{errorMessage || 'Save failed. Please try again.'}
		{/if}
	</div>
{/if}

<style>
	.save-banner {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.6rem 1rem;
		border-radius: 6px;
		font-size: 0.875rem;
		font-weight: 500;
		animation: slide-in 0.15s ease-out;
	}

	.saving {
		background: rgba(var(--accent-rgb), 0.08);
		color: var(--accent);
		border: 1px solid rgba(var(--accent-rgb), 0.2);
	}

	.saved {
		background: rgba(var(--success-rgb), 0.08);
		color: var(--success);
		border: 1px solid rgba(var(--success-rgb), 0.2);
	}

	.error {
		background: rgba(var(--error-rgb), 0.08);
		color: var(--error);
		border: 1px solid rgba(var(--error-rgb), 0.2);
	}

	.spinner {
		display: inline-block;
		width: 0.875rem;
		height: 0.875rem;
		border: 2px solid currentColor;
		border-top-color: transparent;
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	.icon {
		font-size: 0.875rem;
	}

	@keyframes spin {
		to { transform: rotate(360deg); }
	}

	@keyframes slide-in {
		from { opacity: 0; transform: translateY(-4px); }
		to { opacity: 1; transform: translateY(0); }
	}
</style>
