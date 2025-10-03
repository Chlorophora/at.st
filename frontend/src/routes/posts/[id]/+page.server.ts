import { error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import type { Post, Comment, Board } from 'src/app'; // `src/app` は `../../app` など相対パスが適切かもしれません
import { PUBLIC_API_BASE_URL } from '$env/static/public';

export const load: PageServerLoad = async ({ params, fetch, parent }) => {
	try {
		await parent(); // 親レイアウトのload関数を実行し、セッション情報などを継承

		// 1. メインのスレッドデータをまず取得し、board_idを確定させます
		const postRes = await fetch(`${PUBLIC_API_BASE_URL}/posts/${params.id}`);
		if (!postRes.ok) {
			if (postRes.status === 404) throw error(404, '指定されたスレッドが見つかりません。');
			throw error(postRes.status, 'スレッドの読み込みに失敗しました。');
		}

		const postData = await postRes.json().catch(() => {
			throw error(500, 'スレッド情報のAPIレスポンスがJSON形式ではありません。');
		});

		if (!postData?.post) {
			throw error(500, 'スレッド情報のAPIレスポンスの形式が不正です。');
		}

		const post: Post = {
			...postData.post,
			can_moderate: postData.can_moderate
		};

		if (!post.board_id) {
			throw error(500, 'スレッドデータに所属板ID(board_id)が含まれていません。');
		}

		// 2. 残りの関連データを並列で取得し、パフォーマンスを向上させます
		const [commentsRes, boardRes, boardPostsRes] = await Promise.all([
			fetch(`${PUBLIC_API_BASE_URL}/posts/${params.id}/comments`), // コメント一覧
			fetch(`${PUBLIC_API_BASE_URL}/boards/${post.board_id}`), // 権限情報を含む正しい板情報
			fetch(`${PUBLIC_API_BASE_URL}/boards/${post.board_id}/posts`) // 板のスレッド一覧
		]);

		// 3. 各レスポンスを安全に処理します

		// コメント一覧の処理 (404は許容)
		let comments: Comment[] = [];
		if (commentsRes.ok) {
			const commentData = await commentsRes.json().catch(() => null);
			if (Array.isArray(commentData)) {
				comments = commentData
					.map((item) => {
						if (item && item.comment) {
							return { ...item.comment, can_moderate: item.can_moderate };
						}
						return null;
					})
					.filter((c): c is Comment => c !== null);
			}
		} else if (commentsRes.status !== 404) {
			throw error(commentsRes.status, 'コメントの取得に失敗しました。');
		}

		// 板情報の処理 (これが板BAN権限の解決に重要です)
		if (!boardRes.ok) {
			throw error(boardRes.status, '板情報の読み込みに失敗しました。');
		}
		const boardData = await boardRes.json().catch(() => {
			throw error(500, '板情報のAPIレスポンスがJSON形式ではありません。');
		});
		if (!boardData?.board) {
			throw error(500, '板情報のAPIレスポンス形式が不正です。');
		}
		const board: Board = boardData.board;

		// 板のスレッド一覧の処理 (404は許容)
		let posts: Post[] = [];
		if (boardPostsRes.ok) {
			// /boards/[id] の堅牢な実装に合わせて修正
			const postsData = await boardPostsRes.json().catch(() => null);
			if (Array.isArray(postsData)) {
				posts = postsData.filter(Boolean); // 配列内のnull値を除去
			}
		} else if (boardPostsRes.status !== 404) {
			// 404は「スレッド0件」として正常処理
			const errorData = await boardPostsRes.json().catch(() => ({}));
			throw error(boardPostsRes.status, errorData.error || 'スレッド一覧の取得に失敗しました。');
		}

		return { post, board, comments, posts };
	} catch (e: any) {
		// SvelteKitのerror()ヘルパーが投げるHttpErrorはstatusプロパティを持つ
		if (e.status) {
			throw e; // SvelteKit自身に処理させるため、そのまま再スロー
		}
		// 予期せぬJavaScriptエラー（TypeErrorなど）を捕捉
		console.error('posts/[id]/+page.server.ts で予期せぬエラーが発生しました:', e);
		throw error(500, `ページの読み込み中にサーバーで予期せぬエラーが発生しました: ${e.message}`);
	}
};