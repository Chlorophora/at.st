import { error, redirect } from '@sveltejs/kit';
import type { PageLoad } from './$types';
import type { BanDetails } from '$lib/types';

export const load: PageLoad = async ({ fetch, parent, url }) => {
	const { user } = await parent();

	// このページはログインユーザー専用です。
	if (!user) {
		throw redirect(307, '/auth'); // ログインページにリダイレクト
	}

	const page = parseInt(url.searchParams.get('page') || '1');
	const limit = 100; // 1ページあたりの表示件数

	try {
		// 自分が作成したBANの一覧を取得するAPIを呼び出します。
		// hooks.server.ts のプロキシを経由させるため、相対パスを使用します。
		const response = await fetch(`/api/me/bans?page=${page}&limit=${limit}`);

		if (!response.ok) {
			const errorData = await response.json().catch(() => ({}));
			throw error(response.status, errorData.error || 'BAN一覧の取得に失敗しました。');
		}
		const data: { items: BanDetails[]; total_count: number } = await response.json();

		return {
			bans: data.items,
			totalCount: data.total_count,
			currentPage: page,
			limit: limit
		};
	} catch (e: any) {
		// fetch自体のエラー（ネットワークエラーなど）を処理します。
		throw error(500, e.message || 'サーバーとの通信に失敗しました。');
	}
};