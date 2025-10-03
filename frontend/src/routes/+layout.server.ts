import type { LayoutServerLoad } from './$types';
import type { User } from '../app';

/**
 * すべてのページのサーバーサイドレンダリング前に実行されるload関数です。
 * バックエンドにユーザー情報を問い合わせ、結果をページデータとして子コンポーネントに渡します。
 */
export const load: LayoutServerLoad = async ({ fetch }) => {
	try {
		// hooks.server.ts のプロキシを経由してAPIを呼び出すため、相対パスを使用します。
		const response = await fetch('/api/auth/me');

		if (response.ok) {
			const user: User = await response.json();
			return { user }; // ログイン済みの場合はユーザー情報を返す
		}
	} catch (e) {
		// サーバーサイドで起動時にバックエンドがまだ準備できていない場合など、fetchが失敗することがある
		// console.error('Failed to fetch user status in root layout:', e);
	}
	return { user: null }; // 未ログインまたはエラー時はnullを返す
};