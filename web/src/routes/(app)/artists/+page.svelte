<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { getArtists } from '$lib/api';
	import type { Artist } from '$lib/types';

	const PAGE_SIZE = 50;

	let items: Artist[] = $state([]);
	let total = $state(0);
	let offset = $state(0);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let search = $state('');

	let filtered: Artist[] = $derived(
		search.trim()
			? items.filter((a) => a.name.toLowerCase().includes(search.trim().toLowerCase()))
			: items
	);

	async function load(newOffset = 0) {
		loading = true;
		error = null;
		try {
			const res = await getArtists({ limit: PAGE_SIZE, offset: newOffset });
			items = res.items;
			total = res.total;
			offset = newOffset;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load artists';
		} finally {
			loading = false;
		}
	}

	function prevPage() {
		if (offset > 0) load(Math.max(0, offset - PAGE_SIZE));
	}

	function nextPage() {
		if (offset + PAGE_SIZE < total) load(offset + PAGE_SIZE);
	}

	onMount(() => load());
</script>

<div class="catalog-page">
	<header class="catalog-header">
		<h1>Artists</h1>
		<span class="total-badge">{total} total</span>
	</header>

	<div class="search-bar">
		<input
			type="search"
			placeholder="Filter artists…"
			bind:value={search}
			aria-label="Filter artists"
		/>
	</div>

	{#if loading}
		<div class="state-message">Loading artists…</div>
	{:else if error}
		<div class="state-message error">{error}</div>
	{:else if filtered.length === 0}
		<div class="state-message">No artists found.</div>
	{:else}
		<ul class="catalog-list" role="list">
			{#each filtered as artist (artist.id)}
				<li class="catalog-item">
					<button
						class="catalog-row"
						onclick={() => goto(`/artists/${artist.id}`)}
						aria-label="View {artist.name}"
					>
						<span class="item-name">{artist.name}</span>
						<span class="item-meta">
							{#if artist.monitored}
								<span class="badge monitored">Monitored</span>
							{/if}
							<span class="badge status">{artist.status}</span>
						</span>
					</button>
				</li>
			{/each}
		</ul>

		{#if !search.trim()}
			<div class="pagination">
				<button onclick={prevPage} disabled={offset === 0}>← Previous</button>
				<span>{offset + 1}–{Math.min(offset + PAGE_SIZE, total)} of {total}</span>
				<button onclick={nextPage} disabled={offset + PAGE_SIZE >= total}>Next →</button>
			</div>
		{/if}
	{/if}
</div>

<style>
	.catalog-page {
		max-width: 900px;
		margin: 0 auto;
	}

	.catalog-header {
		display: flex;
		align-items: baseline;
		gap: 0.75rem;
		margin-bottom: 1.25rem;
	}

	.catalog-header h1 {
		font-size: 2rem;
		font-weight: 800;
		margin: 0;
	}

	.total-badge {
		font-size: 0.85rem;
		color: var(--text-secondary);
	}

	.search-bar {
		margin-bottom: 1.25rem;
	}

	.search-bar input {
		width: 100%;
		max-width: 420px;
		padding: 0.55rem 0.85rem;
		border: 1px solid var(--border-color);
		border-radius: 6px;
		background: var(--bg-secondary);
		color: var(--text-primary);
		font-size: 0.95rem;
	}

	.search-bar input:focus {
		outline: 2px solid var(--accent);
		outline-offset: 2px;
	}

	.state-message {
		padding: 2rem 0;
		text-align: center;
		color: var(--text-secondary);
	}

	.state-message.error {
		color: var(--error);
	}

	.catalog-list {
		list-style: none;
		padding: 0;
		margin: 0;
		border: 1px solid var(--border-color);
		border-radius: 8px;
		overflow: hidden;
	}

	.catalog-item + .catalog-item {
		border-top: 1px solid var(--border-color);
	}

	.catalog-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: 0.85rem 1.1rem;
		background: var(--bg-secondary);
		border: none;
		cursor: pointer;
		text-align: left;
		color: var(--text-primary);
		transition: background 0.12s;
	}

	.catalog-row:hover {
		background: color-mix(in srgb, var(--accent) 8%, var(--bg-secondary));
	}

	.item-name {
		font-weight: 600;
		font-size: 1rem;
	}

	.item-meta {
		display: flex;
		gap: 0.5rem;
		align-items: center;
	}

	.badge {
		font-size: 0.72rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		padding: 0.2rem 0.5rem;
		border-radius: 4px;
		background: var(--bg-primary);
		color: var(--text-secondary);
	}

	.badge.monitored {
		background: rgba(var(--success-rgb), 0.15);
		color: var(--success);
	}

	.pagination {
		display: flex;
		align-items: center;
		gap: 1rem;
		justify-content: center;
		margin-top: 1.5rem;
	}

	.pagination button {
		padding: 0.45rem 0.9rem;
		border: 1px solid var(--border-color);
		border-radius: 6px;
		background: var(--bg-secondary);
		color: var(--text-primary);
		cursor: pointer;
		font-size: 0.9rem;
		transition: background 0.12s;
	}

	.pagination button:hover:not(:disabled) {
		background: color-mix(in srgb, var(--accent) 10%, var(--bg-secondary));
	}

	.pagination button:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.pagination span {
		font-size: 0.9rem;
		color: var(--text-secondary);
	}
</style>

