<script lang="ts">
	interface Props {
		message: string;
		onretry?: () => void;
	}

	let { message, onretry }: Props = $props();
</script>

<div class="error-message" role="alert" aria-live="assertive">
	<span class="icon" aria-hidden="true">⚠</span>
	<p class="text">{message}</p>
	{#if onretry}
		<button type="button" class="retry-btn" onclick={onretry}>Try again</button>
	{/if}
</div>

<style>
	.error-message {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 1rem 1.25rem;
		background: rgba(182, 66, 46, 0.07);
		border: 1px solid rgba(182, 66, 46, 0.25);
		border-radius: 8px;
		color: var(--error, #b6422e);
	}

	.icon {
		font-size: 1.1rem;
		flex-shrink: 0;
	}

	.text {
		margin: 0;
		font-size: 0.9rem;
		flex: 1;
	}

	.retry-btn {
		padding: 0.35rem 0.85rem;
		border: 1px solid rgba(182, 66, 46, 0.35);
		border-radius: 6px;
		background: transparent;
		color: var(--error, #b6422e);
		cursor: pointer;
		font-size: 0.8rem;
		font-weight: 600;
		white-space: nowrap;
		transition: background 0.12s;
	}

	.retry-btn:hover {
		background: rgba(182, 66, 46, 0.1);
	}
</style>
