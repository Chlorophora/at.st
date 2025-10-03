import type { RequestHandler } from './$types';

/**
 * 専ブラが板のルート (`/{boardId}/`) にアクセスした際のGETリクエストを処理します。
 *
 * 以前の実装では、Webブラウザの利便性のために `/boards/{boardId}` へリダイレクトしていましたが、
 * このリダイレクトが、投稿成功後にCookieを保存しようとしている専ブラの動作を妨害する
 * 原因となっている可能性が高いです。
 *
 * この修正では、リダイレクトを削除し、専ブラの動作を妨げない最小限の応答を返すようにします。
 */
export const GET: RequestHandler = () => {
	// 専ブラはレスポンスボディを通常無視するため、内容は重要ではありません。
	// 重要なのは、リダイレクトではない正常な 200 OK レスポンスを返すことです。
	return new Response('OK', {
		status: 200,
		headers: { 'Content-Type': 'text/plain' }
	});
};