export type PermissionLevel = 'admin' | 'read_only';

export interface FormsLoginResponse {
	authenticated: boolean;
	username: string;
	permission_level: PermissionLevel;
}

export interface FormsLogoutResponse {
	logged_out: boolean;
}

export interface AppearanceSettings {
	theme_mode: 'system' | 'dark' | 'light';
	mobile_breakpoint_px: number;
	mobile_compact_layout: boolean;
	touch_targets_optimized: boolean;
	keyboard_shortcuts_enabled: boolean;
	shortcut_profile: 'standard' | 'vim' | 'emacs';
	bulk_operations_enabled: boolean;
	bulk_selection_limit: number;
	bulk_action_confirmation: boolean;
	advanced_filtering_enabled: boolean;
	default_filter_operator: 'and' | 'or';
	max_filter_clauses: number;
	filter_history_enabled: boolean;
	filter_history_limit: number;
}

export interface AppearanceErrorResponse {
	error: string;
}

export interface SseMessage<T = unknown> {
	sequence?: number;
	tick?: number;
	status?: string;
	queue?: T;
	processing?: T;
	tasks?: T;
}

export interface GenericListResponse {
	items: unknown[];
	total: number;
	limit?: number;
	offset?: number;
}

export interface ActivityItem {
	id: string;
	name: string;
	state: string;
	progress_percent: number;
}

export interface ActivityListResponse {
	items: ActivityItem[];
	total: number;
}

export interface SystemTask {
	id: string;
	name: string;
	schedule_seconds: number;
	enabled: boolean;
	status: string;
}

export interface SystemTasksResponse {
	items: SystemTask[];
	total: number;
	max_concurrent_jobs: number;
}

export interface PaginatedResponse<T> {
	items: T[];
	total: number;
	limit: number;
	offset: number;
}

export interface Artist {
	id: string;
	name: string;
	foreign_artist_id: string | null;
	status: string;
	monitored: boolean;
	path: string | null;
}

export interface Album {
	id: string;
	artist_id: string;
	foreign_album_id: string | null;
	title: string;
	release_date: string | null;
	album_type: string | null;
	status: string;
	monitored: boolean;
}

export interface Track {
	id: string;
	album_id: string;
	artist_id: string;
	foreign_track_id: string | null;
	title: string;
	track_number: number | null;
	duration_ms: number | null;
	has_file: boolean;
	monitored: boolean;
}