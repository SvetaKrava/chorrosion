<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { getArtist, getArtistAlbums } from '$lib/api';
	import type { Artist, Album } from '$lib/types';

	let artist = $state<Artist | null>(null);
	let albums: Album[] = $state([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	async function load() {
		loading = true;
		error = null;
		const id = $page.params.id;
		if (!id) { error = 'Invalid artist ID'; loading = false; return; }
		try {
			const [a, albumsRes] = await Promise.all([getArtist(id), getArtistAlbums(id)]);
			artist = a;
			albums = albumsRes.items;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load artist';
		} finally {
			loading = false;
		}
	}

	onMount(() => load());
</script>

<div class="detail-page">
	{#if loading}
		<div class="state-message">Loading…</div>
	{:else if error}
		<div class="state-message error">{error}</div>
	{:else if artist}
		<div class="back-nav">
			<button onclick={() => goto('/artists')} class="back-btn">← Artists</button>
		</div>

		<header class="detail-header">
			<h1>{artist.name}</h1>
			<div class="detail-meta">
				{#if artist.monitored}
					<span class="badge monitored">Monitored</span>
				{/if}
				<span class="badge status">{artist.status}</span>
				{#if artist.path}
					<span class="path" title={artist.path}>{artist.path}</span>
				{/if}
			</div>
		</header>

		<section class="albums-section">
			<h2>Albums <span class="count">({albums.length})</span></h2>
			{#if albums.length === 0}
				<div class="state-message">No albums found for this artist.</div>
			{:else}
				<ul class="catalog-list" role="list">
					{#each albums as album (album.id)}
						<li class="catalog-item">
							<button
								class="catalog-row"
								onclick={() => goto(`/albums/${album.id}`)}
								aria-label="View album {album.title}"
							>
								<div class="album-info">
									<span class="item-name">{album.title}</span>
									{#if album.release_date}
										<span class="item-year">{album.release_date.slice(0, 4)}</span>
									{/if}
								</div>
								<span class="item-meta">
									{#if album.album_type}
										<span class="badge">{album.album_type}</span>
									{/if}
									{#if album.monitored}
										<span class="badge monitored">Monitored</span>
									{/if}
									<span class="badge status">{album.status}</span>
								</span>
							</button>
						</li>
					{/each}
				</ul>
			{/if}
		</section>
	{/if}
</div>

<style>
	.detail-page {
		max-width: 900px;
		margin: 0 auto;
	}

	.back-btn {
		background: none;
		border: none;
		color: var(--accent);
		cursor: pointer;
		font-size: 0.9rem;
		padding: 0;
		margin-bottom: 1.25rem;
		display: inline-block;
	}

	.back-btn:hover {
		text-decoration: underline;
	}

	.detail-header {
		margin-bottom: 2rem;
	}

	.detail-header h1 {
		font-size: 2.4rem;
		font-weight: 800;
		margin: 0 0 0.5rem;
	}

	.detail-meta {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		align-items: center;
	}

	.path {
		font-size: 0.8rem;
		color: var(--text-secondary);
		font-family: monospace;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		max-width: 300px;
	}

	.albums-section h2 {
		font-size: 1.3rem;
		font-weight: 700;
		margin-bottom: 1rem;
	}

	.count {
		font-weight: 400;
		color: var(--text-secondary);
		font-size: 1rem;
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

	.album-info {
		display: flex;
		align-items: baseline;
		gap: 0.5rem;
	}

	.item-name {
		font-weight: 600;
		font-size: 1rem;
	}

	.item-year {
		font-size: 0.85rem;
		color: var(--text-secondary);
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
</style>
