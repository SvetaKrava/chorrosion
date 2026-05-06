<script lang="ts">
	interface Props {
		message: string;
		actionLabel?: string;
		onaction?: () => void;
	}

	let { message, actionLabel, onaction }: Props = $props();
</script>

<div class="empty-state" role="status">
	<p class="message">{message}</p>
	{#if actionLabel && onaction}
		<button class="action-btn" onclick={onaction}>{actionLabel}</button>
	{/if}
</div>

<style>
	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 1rem;
		padding: 3rem 1rem;
		text-align: center;
		color: var(--text-secondary, #777);
		border: 1px dashed var(--border-color, rgba(15, 26, 31, 0.15));
		border-radius: 8px;
	}

	.message {
		margin: 0;
		font-size: 0.95rem;
	}

	.action-btn {
		padding: 0.55rem 1.25rem;
		border: 1px solid var(--border-color, rgba(15, 26, 31, 0.2));
		border-radius: 6px;
		background: transparent;
		cursor: pointer;
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--accent, #0f7b6c);
		border-color: var(--accent, #0f7b6c);
		transition: background 0.12s;
	}

	.action-btn:hover {
		background: rgba(15, 123, 108, 0.06);
	}
</style>
