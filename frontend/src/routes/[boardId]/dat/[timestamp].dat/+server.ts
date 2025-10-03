import type { RequestHandler } from './$types';
import { generateDat, createSjisResponse } from '$lib/server/monacoin';
import type { Post, Comment } from '../../../../app';

/**
 * 専ブラからのdatファイル取得リクエストに応答するエンドポイントです。
 * 多くの専ブラは、SETTING.TXTに特別な指定がない場合、
 * (板URL)/dat/(タイムスタンプ).dat という形式でスレッドの内容を要求します。
 * このエンドポイントは、その標準的な動作に合致します。
 */
export const GET: RequestHandler = async ({ params, fetch, setHeaders }) => {
	const { boardId, timestamp } = params;

	// params.timestamp には "1754399071.dat" のような文字列が入ってくるため、
	// ".dat" を除去して数値に変換します。
	const requestedTimestamp = parseInt(timestamp.replace('.dat', ''), 10);

	if (!boardId || isNaN(requestedTimestamp)) {
		return createSjisResponse('エラー: 板IDまたはスレッドIDが指定されていません。\n', 'text/plain', 400);
	}

	try {
		// 1. 板に所属するスレッドの一覧を取得します。
		const boardPostsRes = await fetch(`/api/boards/${boardId}/posts`);

		if (!boardPostsRes.ok) {
			return createSjisResponse(
				`エラー: 板のスレッド一覧が取得できませんでした (Code: ${boardPostsRes.status})\n`,
				'text/plain',
				boardPostsRes.status
			);
		}

		const posts: Post[] = await boardPostsRes.json();

		// 2. タイムスタンプIDに一致するスレッドを一覧から探し出します。
		const targetPost = posts.find(
			(p) => Math.floor(new Date(p.created_at).getTime() / 1000) === requestedTimestamp
		);

		if (!targetPost) {
			return createSjisResponse('エラー: 指定されたスレッドが見つかりません。\n', 'text/plain', 404);
		}

		// 3. 解決した連番IDを使ってコメントを取得します。
		const commentsRes = await fetch(`/api/posts/${targetPost.id}/comments`);
		if (!commentsRes.ok) {
			return createSjisResponse(
				`エラー: コメントの読み込みに失敗しました (Code: ${commentsRes.status})\n`,
				'text/plain',
				commentsRes.status
			);
		}

		const commentsData: { comment: Comment }[] = await commentsRes.json();
		const comments: Comment[] = commentsData.map((item) => item.comment);

		const datText = generateDat(targetPost, comments);

		setHeaders({ 'Cache-Control': 'public, max-age=60, s-maxage=60' });

		return createSjisResponse(datText, 'text/plain');
	} catch (e) {
		console.error(`Critical error generating .dat for board ${boardId}, timestamp ${timestamp}:`, e);
		return createSjisResponse('エラー: サーバー内部で問題が発生しました\n', 'text/plain', 500);
	}
};

/**
 * 一部の専ブラがCookie認証時に、スレッドの.datファイルに対して直接POSTリクエストを
 * 送信する特殊な挙動に対応するためのフォールバックハンドラです。
 *
 * このハンドラは、予期せず.datエンドポイントに来た投稿リクエストを捕捉し、
 * 正規の投稿処理エンドポイントである `/bbs.cgi` へサーバーサイドで転送（プロキシ）します。
 */