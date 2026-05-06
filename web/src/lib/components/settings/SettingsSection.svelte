<script lang="ts">
	interface Props {
		title: string;
		description?: string;
		children: import('svelte').Snippet;
		actions?: import('svelte').Snippet;
	}

	let { title, description, children, actions }: Props = $props();
</script>

<section class="settings-section">
	<header class="section-header">
		<div class="section-meta">
			<h2 class="section-title">{title}</h2>
			{#if description}
				<p class="section-description">{description}</p>
			{/if}
		</div>
		{#if actions}
			<div class="section-actions">
				{@render actions()}
			</div>
		{/if}
	</header>

	<div class="section-body">
		{@render children()}
	</div>
</section>

<style>
	.settings-section {
		display: grid;
		gap: 1.5rem;
	}

	.section-header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 1rem;
	}

	.section-meta {
		flex: 1;
		min-width: 0;
	}

	.section-title {
		font-size: 1.5rem;
		font-weight: 700;
		margin: 0 0 0.25rem 0;
		line-height: 1.2;
	}

	.section-description {
		margin: 0;
		color: var(--text-secondary);
		font-size: 0.9rem;
		line-height: 1.5;
	}

	.section-actions {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex-shrink: 0;
	}
</style>
