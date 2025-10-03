import { error, fail } from '@sveltejs/kit';
import type { PageServerLoad, Actions } from './$types';
import { PRIVATE_BACKEND_API_URL } from '$env/static/private';

export const load: PageServerLoad = async ({ fetch, parent }) => {
	const { user } = await parent();

	// ユーザーがログインしていない場合は、何もせずに既存のデータを返す
	if (!user) {
		return {};
	}

	try {
		// バックエンドにレベルアップ可能かどうかのステータスを問い合わせる
		// hooks.server.ts の handleFetch がクッキーを付与してくれる
		const response = await fetch(`${PRIVATE_BACKEND_API_URL}/api/level-up/status`);

		if (!response.ok) {
			const errorData = await response.json().catch(() => ({}));
			// バックエンドからのエラーをクライアントに伝える
			throw error(response.status, errorData.error || 'レベルアップステータスの取得に失敗しました。');
		}

		const levelUpStatus = await response.json();

		return {
			levelUpStatus
		};
	} catch (e: any) {
		// fetch自体が失敗した場合や、上記でthrowされたエラーをキャッチ
		console.error('Failed to fetch level-up status:', e);
		throw error(e.status || 500, e.body?.message || 'サーバーとの通信中にエラーが発生しました。');
	}
};

export const actions: Actions = {
	default: async ({ request, fetch, getClientAddress }) => {
		const formData = await request.formData();
		const turnstileToken = formData.get('turnstile_token');
		const fingerprintDataString = formData.get('fingerprint_data');

		// --- 診断コード ---
		console.log('[DIAG] Server action received.');
		console.log(`[DIAG]   - turnstileToken exists: ${!!turnstileToken}`);
		console.log(`[DIAG]   - fingerprintDataString exists: ${!!fingerprintDataString}`);
		// --- 診断コード終 ---

		// フロントエンドからのデータが不足している場合はエラー
		if (!turnstileToken || !fingerprintDataString) {
			return fail(400, {
				success: false,
				message: '検証データの送信に失敗しました。ページを再読み込みしてもう一度お試しください。'
			});
		}

		try {
			// バックエンドに送信するペイロードを構築
			const payload: { [key: string]: any } = {
				turnstile_token: turnstileToken as string,
				fingerprintData: JSON.parse(fingerprintDataString as string)
			};

			// --- ステップ1: /preflight を呼び出して検証し、トークンを取得 ---
			const preflightResponse = await fetch(`${PRIVATE_BACKEND_API_URL}/api/level-up/preflight`, {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify(payload)
			});

			const preflightText = await preflightResponse.text();
			console.log('--- Preflight Response ---');
			console.log('Status:', preflightResponse.status, preflightResponse.statusText);
			console.log('Raw Text:', preflightText);
			console.log('--------------------------');

			if (!preflightResponse.ok) {
				let errorMessage = '事前検証に失敗しました。';
				try {
					const errorResult = JSON.parse(preflightText);
					errorMessage = errorResult.message || errorResult.error || errorResult.details || errorMessage;
				} catch (e) {}
				return fail(preflightResponse.status, { success: false, message: errorMessage });
			}

			const preflightResult = JSON.parse(preflightText);
			const levelUpToken = preflightResult.level_up_token;

			if (!levelUpToken) {
				return fail(500, { success: false, message: '検証トークンの取得に失敗しました。' });
			}

			// --- ステップ2: /finalize を呼び出してレベルアップを完了 ---
			const finalizeResponse = await fetch(`${PRIVATE_BACKEND_API_URL}/api/level-up/finalize`, {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ level_up_token: levelUpToken })
			});

			const finalizeText = await finalizeResponse.text();
			console.log('--- Finalize Response ---');
			console.log('Status:', finalizeResponse.status, finalizeResponse.statusText);
			console.log('Raw Text:', finalizeText);
			console.log('-------------------------');

			if (!finalizeResponse.ok) {
				let errorMessage = '最終処理に失敗しました。';
				try {
					const errorResult = JSON.parse(finalizeText);
					errorMessage = errorResult.message || errorResult.error || errorResult.details || errorMessage;
				} catch (e) {}
				return fail(finalizeResponse.status, { success: false, message: errorMessage });
			}

			return { success: true, message: 'レベルアップに成功しました！' };
		} catch (e: any) {
			console.error('Level up action failed:', e);
			return fail(500, { success: false, message: 'サーバーとの通信中にエラーが発生しました。' });
		}
	}
};