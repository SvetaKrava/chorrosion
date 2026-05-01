<script lang="ts">
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { getAlbum, getAlbumTracks, getArtist } from '$lib/api';
	import type { Album, Artist, Track } from '$lib/types';

	let album = $state<Album | null>(null);
	let artist = $state<Artist | null>(null);
	let tracks: Track[] = $state([]);
	let trackTotal = $state(0);
	let loading = $state(true);
	let error = $state<string | null>(null);

	function formatDuration(ms: number | null): string {
		if (ms === null) return '—';
		const totalSec = Math.round(ms / 1000);
		const min = Math.floor(totalSec / 60);
		const sec = totalSec % 60;
		return `${min}:${String(sec).padStart(2, '0')}`;
	}

	async function load(id: string | undefined) {
		loading = true;
		error = null;
		if (!id) { error = 'Invalid album ID'; loading = false; return; }
		try {
			const [a, tracksRes] = await Promise.all([getAlbum(id), getAlbumTracks(id, { limit: 200 })]);
			album = a;
			tracks = tracksRes.items.sort(
				(a, b) => (a.track_number ?? 9999) - (b.track_number ?? 9999)
			);
			trackTotal = tracksRes.total;
			// Fetch the artist name for the back-link breadcrumb
			if (a.artist_id) {
				try {
					artist = await getArtist(a.artist_id);
				} catch {
					// non-critical
				}
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load album';
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		void load($page.params.id);
	});
</script>

<div class="detail-page">
	{#if loading}
		<div class="state-message">Loading…</div>
	{:else if error}
		<div class="state-message error">
			<p>{error}</p>
			<button class="retry-btn" onclick={() => load($page.params.id)}>Try again</button>
		</div>
	{:else if album}
		<div class="breadcrumb">
			<button onclick={() => goto('/albums')} class="back-btn">← Albums</button>
			{#if artist}
				<span class="sep">/</span>
				<button onclick={() => goto(`/artists/${artist?.id}`)} class="back-btn"
					>{artist.name}</button
				>
			{/if}
		</div>

		<header class="detail-header">
			<h1>{album.title}</h1>
			<div class="detail-meta">
				{#if album.release_date}
					<span class="meta-item year">{album.release_date.slice(0, 4)}</span>
				{/if}
				{#if album.album_type}
					<span class="badge">{album.album_type}</span>
				{/if}
				{#if album.monitored}
					<span class="badge monitored">Monitored</span>
				{/if}
				<span class="badge status">{album.status}</span>
			</div>
		</header>

		<section class="tracks-section">
			<h2>Tracks <span class="count">({trackTotal})</span></h2>
			{#if trackTotal > tracks.length}
				<p class="truncation-note">Showing {tracks.length} of {trackTotal}</p>
			{/if}
			{#if tracks.length === 0}
				<div class="state-message">No tracks found for this album.</div>
			{:else}
				<table class="tracks-table">
					<thead>
						<tr>
							<th class="col-num">#</th>
							<th class="col-title">Title</th>
							<th class="col-duration">Duration</th>
							<th class="col-file">File</th>
						</tr>
					</thead>
					<tbody>
						{#each tracks as track (track.id)}
							<tr class:has-file={track.has_file}>
								<td class="col-num">{track.track_number ?? '—'}</td>
								<td class="col-title">{track.title}</td>
								<td class="col-duration">{formatDuration(track.duration_ms)}</td>
								<td class="col-file">
									{#if track.has_file}
										<span class="file-indicator has" aria-label="Has file" title="Has file">✓</span>
									{:else}
										<span class="file-indicator missing" aria-label="Missing file" title="Missing file">✗</span>
									{/if}
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			{/if}
		</section>
	{/if}
</div>

<style>
	.detail-page {
		max-width: 900px;
		margin: 0 auto;
	}

	.breadcrumb {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		margin-bottom: 1.25rem;
	}

	.sep {
		color: var(--text-secondary);
		font-size: 0.9rem;
	}

	.back-btn {
		background: none;
		border: none;
		color: var(--accent);
		cursor: pointer;
		font-size: 0.9rem;
		padding: 0;
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

	.meta-item.year {
		font-size: 1rem;
		color: var(--text-secondary);
		font-weight: 600;
	}

	.tracks-section h2 {
		font-size: 1.3rem;
		font-weight: 700;
		margin-bottom: 0.5rem;
	}

	.truncation-note {
		font-size: 0.82rem;
		color: var(--text-secondary);
		margin: 0 0 1rem;
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

	.retry-btn {
		margin-top: 0.75rem;
		padding: 0.45rem 1rem;
		border: 1px solid var(--border-color);
		border-radius: 6px;
		background: var(--bg-secondary);
		color: var(--text-primary);
		cursor: pointer;
		font-size: 0.875rem;
		transition: background 0.12s;
	}

	.retry-btn:hover {
		background: color-mix(in srgb, var(--accent) 10%, var(--bg-secondary));
	}

	.tracks-table {
		width: 100%;
		border-collapse: collapse;
		border: 1px solid var(--border-color);
		border-radius: 8px;
		overflow: hidden;
	}

	.tracks-table thead tr {
		background: var(--bg-primary);
	}

	.tracks-table th {
		text-align: left;
		padding: 0.65rem 1rem;
		font-size: 0.8rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--text-secondary);
		border-bottom: 1px solid var(--border-color);
	}

	.tracks-table td {
		padding: 0.7rem 1rem;
		font-size: 0.95rem;
		border-bottom: 1px solid var(--border-color);
		background: var(--bg-secondary);
	}

	.tracks-table tbody tr:last-child td {
		border-bottom: none;
	}

	.tracks-table tbody tr:hover td {
		background: color-mix(in srgb, var(--accent) 5%, var(--bg-secondary));
	}

	.col-num {
		width: 3rem;
		color: var(--text-secondary);
		font-variant-numeric: tabular-nums;
	}

	.col-duration {
		width: 5rem;
		color: var(--text-secondary);
		font-variant-numeric: tabular-nums;
	}

	.col-file {
		width: 4rem;
		text-align: center;
	}

	.file-indicator {
		font-size: 0.9rem;
		font-weight: 700;
	}

	.file-indicator.has {
		color: var(--success);
	}

	.file-indicator.missing {
		color: var(--text-secondary);
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
