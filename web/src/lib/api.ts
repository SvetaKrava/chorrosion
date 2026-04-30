import type {
	AppearanceErrorResponse,
	AppearanceSettings,
	Artist,
	Album,
	Track,
	FormsLoginResponse,
	FormsLogoutResponse,
	GenericListResponse,
	PaginatedResponse
} from './types';

const API_BASE = (import.meta.env.VITE_CHORROSION_API_BASE ?? 'http://127.0.0.1:5150').replace(
	/\/$/,
	''
);

export class ApiError extends Error {
	status: number;
	body?: unknown;

	constructor(message: string, status: number, body?: unknown) {
		super(message);
		this.status = status;
		this.body = body;
	}
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
	const response = await fetch(`${API_BASE}${path}`, {
		...init,
		credentials: 'include',
		headers: {
			'Content-Type': 'application/json',
			...(init.headers ?? {})
		}
	});

	let body: unknown;
	const text = await response.text();
	if (text.length > 0) {
		try {
			body = JSON.parse(text);
		} catch {
			body = text;
		}
	}

	if (!response.ok) {
		const fallbackMessage = `Request failed with status ${response.status}`;
		const apiMessage =
			typeof body === 'object' && body !== null && 'error' in body
				? String((body as AppearanceErrorResponse).error)
				: fallbackMessage;
		throw new ApiError(apiMessage, response.status, body);
	}

	return (body ?? {}) as T;
}

export function sseUrl(path: string): string {
	return `${API_BASE}${path}`;
}

export async function login(username: string, password: string): Promise<FormsLoginResponse> {
	const payload = new URLSearchParams({ username, password });
	const response = await fetch(`${API_BASE}/api/v1/auth/forms/login`, {
		method: 'POST',
		credentials: 'include',
		headers: {
			'Content-Type': 'application/x-www-form-urlencoded'
		},
		body: payload.toString()
	});

	let body: unknown;
	const text = await response.text();
	if (text.length > 0) {
		try {
			body = JSON.parse(text);
		} catch {
			body = text;
		}
	}

	if (!response.ok) {
		const errorMessage =
			typeof body === 'object' && body !== null && 'error' in body
				? String((body as { error: unknown }).error)
				: `Login failed (${response.status})`;
		throw new ApiError(errorMessage, response.status, body);
	}

	return (body ?? {}) as FormsLoginResponse;
}

export async function logout(): Promise<FormsLogoutResponse> {
	return request<FormsLogoutResponse>('/api/v1/auth/forms/logout', { method: 'POST' });
}

export async function getAppearanceSettings(): Promise<AppearanceSettings> {
	return request<AppearanceSettings>('/api/v1/settings/appearance');
}

export async function updateAppearanceSettings(
	settings: Partial<AppearanceSettings>
): Promise<AppearanceSettings> {
	return request<AppearanceSettings>('/api/v1/settings/appearance', {
		method: 'PUT',
		body: JSON.stringify(settings)
	});
}

export async function getQueueSnapshot(): Promise<GenericListResponse> {
	return request<GenericListResponse>('/api/v1/activity/queue');
}

export async function getProcessingSnapshot(): Promise<GenericListResponse> {
	return request<GenericListResponse>('/api/v1/activity/processing');
}

export async function getTasksSnapshot(): Promise<GenericListResponse> {
	return request<GenericListResponse>('/api/v1/system/tasks');
}

export async function getArtists(params?: {
	limit?: number;
	offset?: number;
}): Promise<PaginatedResponse<Artist>> {
	const query = new URLSearchParams();
	if (params?.limit !== undefined) query.set('limit', String(params.limit));
	if (params?.offset !== undefined) query.set('offset', String(params.offset));
	const qs = query.toString();
	return request<PaginatedResponse<Artist>>(`/api/v1/artists${qs ? `?${qs}` : ''}`);
}

export async function getArtist(id: string): Promise<Artist> {
	return request<Artist>(`/api/v1/artists/${encodeURIComponent(id)}`);
}

export async function getArtistAlbums(artistId: string): Promise<PaginatedResponse<Album>> {
	return request<PaginatedResponse<Album>>(
		`/api/v1/artists/${encodeURIComponent(artistId)}/albums`
	);
}

export async function getAlbums(params?: {
	limit?: number;
	offset?: number;
}): Promise<PaginatedResponse<Album>> {
	const query = new URLSearchParams();
	if (params?.limit !== undefined) query.set('limit', String(params.limit));
	if (params?.offset !== undefined) query.set('offset', String(params.offset));
	const qs = query.toString();
	return request<PaginatedResponse<Album>>(`/api/v1/albums${qs ? `?${qs}` : ''}`);
}

export async function getAlbum(id: string): Promise<Album> {
	return request<Album>(`/api/v1/albums/${encodeURIComponent(id)}`);
}

export async function getAlbumTracks(albumId: string): Promise<PaginatedResponse<Track>> {
	return request<PaginatedResponse<Track>>(
		`/api/v1/albums/${encodeURIComponent(albumId)}/tracks`
	);
}
