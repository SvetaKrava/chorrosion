export type PermissionLevel = 'admin' | 'read_only';

export interface FormsLoginResponse {
	authenticated: boolean;
	username: string;
	permission_level: PermissionLevel;
}

export interface FormsLogoutResponse {
	logged_out: boolean;
}

/** Shared paginated response wrapper used for all list endpoints. */
export interface PaginatedResponse<T> {
	items: T[];
	total: number;
	limit: number;
	offset: number;
}

/** Shared API error response shape returned by all endpoints on failure. */
export interface ApiErrorResponse {
	error: string;
}

/** Field-level validation error detail (included in 400/422 responses). */
export interface ApiValidationFieldError {
	field: string;
	message: string;
}

/** Extended error shape for validation failures that include per-field errors. */
export interface ApiValidationError extends ApiErrorResponse {
	fields?: ApiValidationFieldError[];
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

export type AppearanceErrorResponse = ApiErrorResponse;

export interface DownloadClient {
	id: string;
	name: string;
	client_type: string;
	base_url: string;
	username: string | null;
	category: string | null;
	enabled: boolean;
	has_password: boolean;
}

export type ListDownloadClientsResponse = PaginatedResponse<DownloadClient>;

export interface CreateDownloadClientRequest {
	name: string;
	client_type: string;
	base_url: string;
	username?: string | null;
	password?: string | null;
	category?: string | null;
	enabled?: boolean;
}

export interface UpdateDownloadClientRequest {
	name?: string;
	client_type?: string;
	base_url?: string;
	username?: string;
	password?: string;
	category?: string;
	enabled?: boolean;
}

export type DownloadClientErrorResponse = ApiErrorResponse;

export type SettingsBulkAction = 'enable' | 'disable' | 'delete';

export interface SettingsBulkRequest {
	action: SettingsBulkAction;
	ids: string[];
}

export interface SettingsBulkItemResult {
	id: string;
	success: boolean;
	error?: string;
}

export interface SettingsBulkResponse {
	results: SettingsBulkItemResult[];
}

export interface Indexer {
	id: string;
	name: string;
	base_url: string;
	protocol: string;
	enabled: boolean;
	has_api_key: boolean;
}

export type ListIndexersResponse = PaginatedResponse<Indexer>;

export interface CreateIndexerRequest {
	name: string;
	base_url: string;
	protocol: string;
	api_key?: string | null;
	enabled?: boolean;
}

export interface UpdateIndexerRequest {
	name?: string;
	base_url?: string;
	protocol?: string;
	api_key?: string;
	enabled?: boolean;
}

export type IndexerErrorResponse = ApiErrorResponse;

export interface TestIndexerRequest {
	name: string;
	base_url: string;
	protocol: string;
	api_key?: string | null;
}

export interface IndexerCapabilities {
	supports_search: boolean;
	supports_rss: boolean;
	supports_capabilities_detection: boolean;
	supports_categories: boolean;
	supported_categories: string[];
}

export interface TestIndexerResponse {
	success: boolean;
	message: string;
	protocol: string;
	capabilities: IndexerCapabilities;
}

export type IndexerTestErrorResponse = ApiErrorResponse;

export interface QualityProfile {
	id: string;
	name: string;
	allowed_qualities: string[];
	upgrade_allowed: boolean;
	cutoff_quality: string | null;
}

export type ListQualityProfilesResponse = PaginatedResponse<QualityProfile>;

export interface CreateQualityProfileRequest {
	name: string;
	allowed_qualities: string[];
	upgrade_allowed?: boolean;
	cutoff_quality?: string | null;
}

export interface UpdateQualityProfileRequest {
	name?: string;
	allowed_qualities?: string[];
	upgrade_allowed?: boolean;
	cutoff_quality?: string | null;
}

export type QualityProfileErrorResponse = ApiErrorResponse;

export interface MetadataProfile {
	id: string;
	name: string;
	primary_album_types: string[];
	secondary_album_types: string[];
	release_statuses: string[];
}

export type ListMetadataProfilesResponse = PaginatedResponse<MetadataProfile>;

export interface CreateMetadataProfileRequest {
	name: string;
	primary_album_types?: string[];
	secondary_album_types?: string[];
	release_statuses?: string[];
}

export interface UpdateMetadataProfileRequest {
	name?: string;
	primary_album_types?: string[];
	secondary_album_types?: string[];
	release_statuses?: string[];
}

export type MetadataProfileErrorResponse = ApiErrorResponse;

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
