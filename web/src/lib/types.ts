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

export interface SseMessage<T> {
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