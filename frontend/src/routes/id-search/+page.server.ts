import type { PageServerLoad } from './$types';
import { error } from '@sveltejs/kit';
import { PUBLIC_API_BASE_URL } from '$env/static/public';

export const load: PageServerLoad = async ({ fetch, url }) => {
	const user_part = url.searchParams.get('user_part');
	const ip_part = url.searchParams.get('ip_part');
	const device_part = url.searchParams.get('device_part');
	const logic = url.searchParams.get('logic') || 'and';
	const sort = url.searchParams.get('sort') || 'time_desc';

	// フォームの初期値を設定
	const formState = {
		user_part: user_part || '',
		ip_part: ip_part || '',
		device_part: device_part || '',
		logic,
		sort
	};

	// 検索パラメータが一つもなければ、APIを叩かずに初期状態を返す
	if (!user_part && !ip_part && !device_part) {
		return {
			...formState,
			historyResponse: null,
			searchError: null
		};
	}

	const query = new URLSearchParams({
		user_part: user_part || '',
		ip_part: ip_part || '',
		device_part: device_part || '',
		logic,
		sort
	});

	try {
		const res = await fetch(`${PUBLIC_API_BASE_URL}/history/by-id-parts?${query.toString()}`);

		if (!res.ok) {
			const errorJson = await res.json();
			// バックエンドからのエラーメッセージをフロントエンドに渡す
			return {
				...formState,
				historyResponse: null,
				searchError: errorJson.message || `エラーが発生しました (HTTP ${res.status})`
			};
		}

		const historyResponse: HistoryResponse = await res.json();
		return { ...formState, historyResponse, searchError: null };
	} catch (e) {
		console.error('Failed to fetch history:', e);
		throw error(500, 'サーバーとの通信に失敗しました。');
	}
};