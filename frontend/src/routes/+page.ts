import type { PageLoad } from './$types';
import { error as svelteKitError } from '@sveltejs/kit';

export const load: PageLoad = async ({ fetch, parent, url }) => {
	// 親レイアウトの`load`が完了するのを待ち、ユーザーセッションの変更を検知できるようにする
	await parent();

	try {
		const page = url.searchParams.get('page') || '1';
		// hooks.server.ts のプロキシを経由してAPIを呼び出すため、相対パスを使用します。
		// これにより、サーバーサイドでもクライアントサイドでも同じコードで動作します。
		const response = await fetch(`/api/boards?page=${page}`, {
			credentials: 'include' // ブラウザからAPIを叩く際にCookieを送信するために必要
		});

		if (!response.ok) {
			// APIからのレスポンスがエラーだった場合
			// SvelteKitのerrorヘルパーを使い、ステータスコードとメッセージを渡す
			throw svelteKitError(response.status, `APIからの応答エラー: ${response.statusText}`);
		}

		const paginatedBoards = await response.json();

		// 取得したデータをページコンポーネントに渡す
		return {
			paginatedBoards
		};
	} catch (error) {
		// svelteKitErrorによってスローされたエラーは、SvelteKitが適切に処理するのでそのまま再スロー
		if (error && typeof error === 'object' && 'status' in error) {
			throw error;
		}
		console.error('板データの取得に失敗しました:', error);
		// 予期せぬエラーが発生した場合もerrorヘルパーで処理する
		throw svelteKitError(500, (error instanceof Error ? error.message : '掲示板のデータを取得できませんでした。'));
	}
};