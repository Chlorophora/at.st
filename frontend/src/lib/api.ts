import type { Fetch } from '@sveltejs/kit';

/**
 * バックエンドAPIとの通信を簡略化するヘルパー関数。
 *
 * @param fetch SvelteKitの`load`関数から渡される`fetch`インスタンス。サーバーサイドとクライアントサイドの両方で正しく動作するために必要。
 * @param endpoint APIのエンドポイント (例: 'boards', 'admin/users')。'/api/'は自動的に付与されます。
 * @param method HTTPメソッド (デフォルトは 'GET')。
 * @param body リクエストボディとして送信するオブジェクト。
 * @returns fetchのレスポンスPromise。
 */
export async function api(
	fetch: Fetch,
	endpoint: string,
	method: 'GET' | 'POST' | 'DELETE' | 'PATCH' = 'GET',
	body?: object
): Promise<Response> {
	const options: RequestInit = {
		method,
		headers: { 'Content-Type': 'application/json' }
	};

	if (body) { options.body = JSON.stringify(body); }

	return fetch(`/api/${endpoint}`, options);
}