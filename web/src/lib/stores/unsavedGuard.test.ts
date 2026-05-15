import { beforeEach, describe, expect, it, vi } from 'vitest';

type BeforeNavigateHandler = (args: {
	cancel: () => void;
	to?: { url: URL } | null;
}) => void;

const { mockGoto, navigationState } = vi.hoisted(() => ({
	mockGoto: vi.fn(async (_href: string) => {}),
	navigationState: {
		handler: null as BeforeNavigateHandler | null
	}
}));

vi.mock('$app/navigation', () => ({
	beforeNavigate: (handler: BeforeNavigateHandler) => {
		navigationState.handler = handler;
	},
	goto: mockGoto
}));

import { useUnsavedGuard } from '$lib/stores/unsavedGuard';

describe('useUnsavedGuard', () => {
	beforeEach(() => {
		navigationState.handler = null;
		mockGoto.mockClear();
	});

	it('blocks navigation and stores pending href when dirty', () => {
		const onBlocked = vi.fn();
		const guard = useUnsavedGuard(onBlocked);
		guard.markDirty();
		const cancel = vi.fn();

		navigationState.handler?.({
			cancel,
			to: { url: new URL('http://localhost/settings/indexers') }
		});

		expect(cancel).toHaveBeenCalledOnce();
		expect(onBlocked).toHaveBeenCalledOnce();
		expect(guard.hasPendingNavigation).toBe(true);
	});

	it('confirmNavigation resumes pending navigation', async () => {
		const guard = useUnsavedGuard();
		guard.markDirty();

		navigationState.handler?.({
			cancel: vi.fn(),
			to: { url: new URL('http://localhost/settings/quality-profiles') }
		});

		await guard.confirmNavigation();

		expect(mockGoto).toHaveBeenCalledWith('http://localhost/settings/quality-profiles');
		expect(guard.hasPendingNavigation).toBe(false);
		expect(guard.isDirty).toBe(false);
	});

	it('discardNavigation clears pending navigation without navigating', () => {
		const guard = useUnsavedGuard();
		guard.markDirty();

		navigationState.handler?.({
			cancel: vi.fn(),
			to: { url: new URL('http://localhost/settings/download-clients') }
		});

		guard.discardNavigation();

		expect(guard.hasPendingNavigation).toBe(false);
		expect(mockGoto).not.toHaveBeenCalled();
	});

	it('markClean prevents navigation from being blocked', () => {
		const onBlocked = vi.fn();
		const guard = useUnsavedGuard(onBlocked);
		guard.markDirty();
		guard.markClean();
		const cancel = vi.fn();

		navigationState.handler?.({
			cancel,
			to: { url: new URL('http://localhost/settings/metadata-profiles') }
		});

		expect(cancel).not.toHaveBeenCalled();
		expect(onBlocked).not.toHaveBeenCalled();
		expect(guard.hasPendingNavigation).toBe(false);
	});
});
