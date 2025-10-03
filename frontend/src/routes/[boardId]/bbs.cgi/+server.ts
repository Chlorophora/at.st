import type { RequestHandler } from './$types';

/**
 * 一部の専ブラがBBS_CGIを板URLからの相対パスと解釈し、
 * `/{boardId}/bbs.cgi` のようなURLに投稿リクエストを送信する問題に対応するためのエンドポイント。
 *
 * このハンドラは、受け取ったPOSTリクエストを正規の投稿エンドポイントである `/bbs.cgi` へ
 * サーバーサイドで転送（プロキシ）します。
 * これにより、投稿処理のロジックを一元化しつつ、互換性を確保します。
 */
export const POST: RequestHandler = async ({ request, fetch }) => {
	// SvelteKitのサーバーサイドfetchを使い、リクエストを内部的に転送します。
	// 第2引数にオリジナルのRequestオブジェクトを渡すことで、
	// HTTPメソッド、ヘッダー、ボディがすべてそのまま引き継がれます。
	const response = await fetch('/bbs.cgi', request);

	// /bbs.cgi からのレスポンス（成功/失敗を示すHTML）を、
	// そのままクライアントである専用ブラウザに返却します。
	return response;
};