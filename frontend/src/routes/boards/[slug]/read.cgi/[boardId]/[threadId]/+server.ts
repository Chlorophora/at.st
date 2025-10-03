import { redirect } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { generateDat, createSjisResponse } from '$lib/server/monacoin';
import type { Post, Comment } from '../../../../../../app';

export const GET: RequestHandler = async ({ fetch, params, request, setHeaders }) => {
	const acceptHeader = request.headers.get('accept') || '';
	const wantsHtml = /text\/html/.test(acceptHeader);
	const { boardId, threadId } = params;

	if (!boardId || !threadId) {
		if (wantsHtml) {
			throw redirect(302, '/');
		}
		return createSjisResponse('エラー: 板IDまたはスレッドIDが指定されていません。\n', 'text/plain', 400);
	}

	try {
		// 1. 板に所属するスレッドの一覧を取得します。
		const boardPostsRes = await fetch(`/api/boards/${boardId}/posts`);

		if (!boardPostsRes.ok) {
			console.error(
				`[read.cgi] Failed to fetch posts for board ${boardId}. Status: ${boardPostsRes.status}`
			);
			if (wantsHtml) {
				throw redirect(302, '/');
			}
			return createSjisResponse(
				`エラー: 板のスレッド一覧が取得できませんでした (Code: ${boardPostsRes.status})\n`,
				'text/plain',
				boardPostsRes.status
			);
		}

		const posts: Post[] = await boardPostsRes.json();

		// 2. タイムスタンプIDに一致するスレッドを一覧から探し出します。
		const targetTimestamp = Number(threadId);
		const targetPost = posts.find(
			(p) => Math.floor(new Date(p.created_at).getTime() / 1000) === targetTimestamp
		);

		if (!targetPost) {
			console.warn(`[read.cgi] Thread with timestamp ${threadId} not found in board ${boardId}.`);
			if (wantsHtml) {
				throw redirect(302, '/');
			}
			return createSjisResponse('エラー: 指定されたスレッドが見つかりません。\n', 'text/plain', 404);
		}

		// 3. リクエストの種類に応じて処理を分岐します。
		if (wantsHtml) {
			// 【一般ブラウザ向け】解決した連番IDを使って、正しいスレッドページへリダイレクトします。
			throw redirect(302, `/posts/${targetPost.id}`);
		} else {
			// 【専用ブラウザ向け】.datファイル生成処理
			// 解決した連番IDを使ってコメントを取得します。
			const commentsRes = await fetch(`/api/posts/${targetPost.id}/comments`);
			if (!commentsRes.ok) {
				const errorText = await commentsRes.text().catch(() => 'Could not read error body');
				console.error(
					`API error for comments of post ${targetPost.id}: ${commentsRes.status} - ${errorText}`
				);
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
		}
	} catch (e: unknown) {
		// SvelteKitのredirectはエラーをスローするので、それを再スローします
		if (
			typeof e === 'object' &&
			e &&
			'status' in e &&
			typeof e.status === 'number' &&
			e.status >= 300 &&
			e.status < 400
		) {
			throw e;
		}
		// fetch自体の失敗やJSONパースエラーなど、その他の予期せぬエラーを捕捉
		console.error(`An unexpected error occurred in read.cgi handler for thread ${threadId}:`, e);
		if (wantsHtml) {
			throw redirect(302, '/');
		}
		return createSjisResponse('エラー: サーバー内部で問題が発生しました\n', 'text/plain', 500);
	}
};