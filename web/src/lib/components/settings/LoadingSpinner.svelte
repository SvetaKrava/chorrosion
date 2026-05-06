<script lang="ts">
	interface Props {
		label?: string;
		size?: 'sm' | 'md' | 'lg';
	}

	let { label = 'Loading…', size = 'md' }: Props = $props();
</script>

<div class="spinner-wrapper" class:sm={size === 'sm'} class:lg={size === 'lg'} role="status" aria-label={label}>
	<span class="spinner" aria-hidden="true"></span>
	<span class="sr-only">{label}</span>
</div>

<style>
	.spinner-wrapper {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 2rem;
	}

	.spinner-wrapper.sm {
		padding: 0.5rem;
	}

	.spinner-wrapper.lg {
		padding: 4rem;
	}

	.spinner {
		display: inline-block;
		width: 1.5rem;
		height: 1.5rem;
		border: 2px solid rgba(var(--accent-rgb), 0.2);
		border-top-color: var(--accent);
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	.sm .spinner {
		width: 1rem;
		height: 1rem;
	}

	.lg .spinner {
		width: 2.5rem;
		height: 2.5rem;
	}

	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
	}

	@keyframes spin {
		to { transform: rotate(360deg); }
	}
</style>
