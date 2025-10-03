import type { PageServerLoad } from './$types';
import { error } from '@sveltejs/kit';
import type { Post, Board } from '$lib/types';
import { PUBLIC_API_BASE_URL } from '$env/static/public';
 
export const load: PageServerLoad = async ({ fetch, params, parent }) => {
	try {

		await parent(); // 親レイアウトの読み込みを待ち、ユーザー情報を取得

		// 板の詳細情報とスレッド一覧を並列で取得します。
		// APIへのリクエストは、環境変数で指定されたベースURLを使用して絶対パスで指定します。
		const [boardRes, postsRes] = await Promise.all([
			fetch(`${PUBLIC_API_BASE_URL}/boards/${params.id}`),
			fetch(`${PUBLIC_API_BASE_URL}/boards/${params.id}/posts`)
		]);

		// 板が見つからない場合は404エラーを返す
		if (boardRes.status === 404) {
			throw error(404, '指定された板が見つかりません。');
		}
		if (!boardRes.ok) {
			const errorData = await boardRes.json().catch(() => ({}));
			throw error(boardRes.status, errorData.error || '板の情報の取得に失敗しました。');
		}

		const boardData: { board: Board; creator_info: any } = await boardRes.json();

		// `boardData.board` には既に `current_user_is_admin` フラグが含まれているため、そのまま返します。
		const board = boardData.board;
		const creatorInfo = boardData.creator_info || null;

		// スレッド一覧を取得します。
		// スレッドが0件の場合、APIは404を返す可能性があるため、その場合は空の配列として扱う。
		let posts: Post[] = [];
		if (postsRes.ok) {
			// /posts/[id] の実装に基づき、APIはスレッドの配列を直接返すと想定。
			// JSONパースに失敗した場合や、レスポンスが配列でない場合も安全に処理する。
			const postsData = await postsRes.json().catch(() => null);
			if (Array.isArray(postsData)) {
				posts = postsData.filter(Boolean); // 配列内のnull値を除去
			}
		} else if (postsRes.status !== 404) { // 404は「スレッド0件」として正常処理
			// 404 (スレッド0件) 以外のエラーステータスはスローする
			const errorData = await postsRes.json().catch(() => ({}));
			throw error(postsRes.status, errorData.error || 'スレッド一覧の取得に失敗しました。');
		}

		return {
			board,
			posts,
			creatorInfo
			// `board`オブジェクトに`can_moderate`フラグが含まれているため、ここで別途計算する必要はありません。
		};
	} catch (e) {
		// SvelteKitの`error()`関数によってスローされたエラーの場合、
		// そのまま再スローしてSvelteKitのエラーページ表示に任せます。
		if (e && typeof e === 'object' && 'status' in e) {
			throw e;
		}

		// fetchのネットワークエラーなど、予期せぬエラーの場合
		console.error('An unexpected error occurred in load function:', e);
		throw error(500, 'ページの読み込み中に予期せぬエラーが発生しました。');
	}
};