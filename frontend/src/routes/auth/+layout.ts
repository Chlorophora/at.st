import type { LayoutLoad } from './$types';
import { redirect } from '@sveltejs/kit';
import { user as userStore } from '$lib/stores/userStore';
import { browser } from '$app/environment';

/**
 * /auth/ 以下のすべてのページで実行されるload関数です。
 * このレイアウトは、認証状態に応じてユーザーを適切なページにリダイレクトする役割を担います。
 */
export const load: LayoutLoad = async ({ url, fetch }) => {
	// 常に最新の認証状態を直接確認します。
	const response = await fetch('/api/auth/me');

	// 認証済みの場合
	if (response.ok) {
		const user = await response.json();
		userStore.set(user);

		// ログイン済みのユーザーが /auth/ 以下のページにアクセスした場合、
		// ホームページにリダイレクトします。
		throw redirect(303, '/');
	}

	// 未認証の場合
	userStore.set(null);

	// 無効なセッショントークンがブラウザに残っている可能性があるため、削除します。
	// これにより、不要なAPIコールを防ぎます。
	if (browser && response.status === 401) {
		console.log('Invalid session detected (401). Clearing session_token cookie.');
		document.cookie = 'session_token=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT';
	}

	// 未認証なので、そのまま /auth/login や /auth/register ページを表示させるために空のオブジェクトを返します。
	return {};
};