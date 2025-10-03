import type { RequestHandler } from './$types';

/**
 * 専ブラが旧形式のURL (`/boards/123/dat/...` など) にアクセスした場合に、
 * 新形式の正規URL (`/123/dat/...`) へリクエストを内部で転送（プロキシ）するためのハンドラ。
 *
 * 以前はHTTP 301リダイレクトを返していましたが、このリダイレクトが投稿成功直後の
 * Cookie保存処理を妨害する原因となっていたため、サーバーサイドでのプロキシ方式に変更します。
 * これにより、クライアント（専ブラ）はリダイレクトを意識することなく、一貫したURLで通信できます。
 */
export const GET: RequestHandler = async ({ params, fetch }) => {
	const { id: boardId, path: pathSegments } = params;

	// `params.path` は `subject.txt` や `dat/12345.dat` のように `/` で分割された文字列になるため、
	// これを結合して元のパスを復元します。
	const joinedPath = pathSegments; // SvelteKit v1では配列、v2では単一文字列。どちらでも動くように。

	// 転送先の正規URLを構築します。
	const newPath = `/${boardId}/${joinedPath}`;

	// SvelteKitのサーバーサイドfetchを使い、リクエストを内部的に転送します。
	// これにより、専ブラはリダイレクトを経験することなく、目的のコンテンツを取得できます。
	return await fetch(newPath);
};

/**
 * 一部の専ブラが `/boards/...` 形式のURLにいるときに、
 * 相対パスで投稿リクエストを送信してしまう問題に対応するためのフォールバック。
 *
 * このハンドラは、`/boards/{id}/bbs.cgi` のような予期せぬURLに来たPOSTリクエストを捕捉し、
 * 正規の投稿処理エンドポイントである `/bbs.cgi` へサーバーサイドで転送（プロキシ）します。
 */
export const POST: RequestHandler = async ({ request, fetch }) => {
	// SvelteKitのサーバーサイドfetchを使い、リクエストを内部的に転送します。
	return await fetch('/bbs.cgi', request);
};
