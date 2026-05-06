<script lang="ts">
	interface Props {
		open: boolean;
		title?: string;
		message: string;
		confirmLabel?: string;
		cancelLabel?: string;
		destructive?: boolean;
		onconfirm: () => void;
		oncancel: () => void;
	}

	let {
		open,
		title = 'Confirm',
		message,
		confirmLabel = 'Confirm',
		cancelLabel = 'Cancel',
		destructive = false,
		onconfirm,
		oncancel
	}: Props = $props();

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			oncancel();
		}
	}
</script>

{#if open}
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<div
		class="dialog-backdrop"
		role="dialog"
		aria-modal="true"
		aria-labelledby="dialog-title"
		tabindex="-1"
		onkeydown={handleKeydown}
	>
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<div class="dialog-scrim" role="presentation" onclick={oncancel}></div>
		<div class="dialog-panel">
			<h3 class="dialog-title" id="dialog-title">{title}</h3>
			<p class="dialog-message">{message}</p>
			<div class="dialog-actions">
				<button class="btn-cancel" onclick={oncancel}>{cancelLabel}</button>
				<button
					class="btn-confirm"
					class:destructive
					onclick={onconfirm}
				>
					{confirmLabel}
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
		backdrop-filter: blur(2px);
	}

	.dialog-panel {
		position: relative;
		z-index: 1;
		background: var(--paper, #fffdf7);
		border: 1px solid var(--border-color, rgba(15, 26, 31, 0.15));
		border-radius: 12px;
		padding: 1.75rem 2rem;
		max-width: 420px;
		width: calc(100% - 2rem);
		box-shadow: 0 16px 48px rgba(15, 26, 31, 0.18);
		animation: pop-in 0.15s ease-out;
	}

	.dialog-title {
		margin: 0 0 0.75rem 0;
		font-size: 1.1rem;
		font-weight: 700;
	}

	.dialog-message {
		margin: 0 0 1.5rem 0;
		color: var(--text-secondary, #555);
		font-size: 0.9rem;
		line-height: 1.5;
	}

	.dialog-actions {
		display: flex;
		justify-content: flex-end;
		gap: 0.75rem;
	}

	.btn-cancel {
		padding: 0.55rem 1.1rem;
		border: 1px solid var(--border-color, rgba(15, 26, 31, 0.2));
		border-radius: 6px;
		background: transparent;
		cursor: pointer;
		font-size: 0.875rem;
		font-weight: 500;
		color: var(--text-primary, #0f1a1f);
		transition: background 0.12s;
	}

	.btn-cancel:hover {
		background: rgba(0, 0, 0, 0.04);
	}

	.btn-confirm {
		padding: 0.55rem 1.1rem;
		border: none;
		border-radius: 6px;
		background: var(--accent, #0f7b6c);
		color: #fff;
		cursor: pointer;
		font-size: 0.875rem;
		font-weight: 600;
		transition: opacity 0.12s;
	}

	.btn-confirm:hover {
		opacity: 0.88;
	}

	.btn-confirm.destructive {
		background: var(--error, #b6422e);
	}

	@keyframes pop-in {
		from { opacity: 0; transform: scale(0.95); }
		to { opacity: 1; transform: scale(1); }
	}
</style>
