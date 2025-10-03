import type { RequestHandler } from './$types';
import { PUBLIC_API_BASE_URL } from '$env/static/public';
import iconv from 'iconv-lite';
import type { Board } from '$lib/types';

/**
 * 専ブラ（専用ブラウザ）向けに、すべての板一覧をbbsmenu.html形式で提供するエンドポイントです。
 */
export const GET: RequestHandler = async ({ setHeaders, fetch }) => {
	// このエンドポイントは頻繁にアクセスされる可能性があるため、キャッシュを設定してサーバー負荷を軽減します。
	setHeaders({
		'Cache-Control': 'public, max-age=300, s-maxage=300' // 5分キャッシュ
	});

	try {
		// バックエンドAPIから板一覧をすべて取得します。
		// hooks.server.tsのhandleFetchフックにより、このリクエストはバックエンドにプロキシされます。
		const res = await fetch(`${PUBLIC_API_BASE_URL}/boards`);

		let boards: Board[] = [];

		if (res.ok) {
			const responseData = await res.json();
			// APIからのレスポンスが配列、または { items: [...] } 形式であるかを確認します。
			if (Array.isArray(responseData)) {
				boards = responseData;
			} else if (responseData && Array.isArray(responseData.items)) {
				// ページネーションされたレスポンス形式 ({ items: [...] }) に対応します。
				boards = responseData.items;
			} else {
				// 予期せぬ形式のレスポンスが来た場合は、エラーをログに出力します。
				console.error(
					`API response for /boards was not an array or a paginated object. Received:`,
					responseData
				);
			}
		} else {
			console.error(
				`Failed to fetch boards for /boards/bbsmenu.html (status: ${res.status}). Returning empty menu.`
			);
			// APIエラー時も専ブラがクラッシュしないよう、空のメニューを返します。
		}

		// HTML特殊文字をエスケープするヘルパー関数
		const escapeHtml = (text: string | null | undefined): string => {
			if (!text) return '';
			return text
				.replace(/&/g, '&amp;')
				.replace(/</g, '&lt;')
				.replace(/>/g, '&gt;'); // 専ブラ互換性のため、基本的なエスケープに留める
		};

		// 板一覧のリンクを生成します。
		// 専ブラが /boards/bbsmenu.html を基準にURLを解決するため、HREFには相対パス (例: "1/") を指定します。
		// これにより、専ブラは /boards/1/ のように正しいURLを構築します。
		const boardLinks = boards
			.filter((board) => !board.archived_at) // アーカイブ済みの板を除外
			.map((board) => `<A HREF="${board.id}/">${escapeHtml(board.name)}</A>`)
			.join('<br>\n');

		const body = `<B>紅茶</B><br>\n${boardLinks}`;
		const html = `<HTML><HEAD><TITLE>BBS MENU</TITLE></HEAD><BODY>${body}</BODY></HTML>`;

		const encodedBody = iconv.encode(html, 'Shift_JIS');
		const headers = new Headers();
		headers.set('Content-Type', 'text/html; charset=Shift_JIS');
		headers.set('Content-Length', encodedBody.length.toString());

		return new Response(encodedBody, { status: 200, headers });
	} catch (e) {
		console.error(`Critical error generating /boards/bbsmenu.html:`, e);
		// 予期せぬエラーが発生した場合も、空のメニューをShift_JISで返します。
		const errorHtml = `<HTML><HEAD><TITLE>BBS MENU</TITLE></HEAD><BODY></BODY></HTML>`;
		const encodedBody = iconv.encode(errorHtml, 'Shift_JIS');
		const headers = new Headers();
		headers.set('Content-Type', 'text/html; charset=Shift_JIS');
		headers.set('Content-Length', encodedBody.length.toString());
		return new Response(encodedBody, { status: 500, headers });
	}
};