import type { RequestHandler } from './$types';
import { createSjisResponse } from '$lib/server/monacoin';

export const GET: RequestHandler = async ({ setHeaders }) => {
	// このエンドポイントは、専ブラ（専用ブラウザ）からの bbsmenu.html へのリクエストに応答します。
	// 専ブラは、このファイルから板（掲示板）のリストを読み込みます。

	setHeaders({
		'Cache-Control': 'public, max-age=300, s-maxage=300' // 5分キャッシュ
	});

	// このエンドポイントは、専ブラからの bbsmenu.html へのリクエストに応答します。
	// ご要望に基づき、全板取得によるサーバー負荷をなくすため、
	// 意図的に/boards/1/ と /boards/2/ の2つの板を返すようにします。
	// 専ブラに表示する板のリストを定義します。
	// フォーマット: <A HREF="板のURL">板の名前</A>
	// URLを絶対パスにすることでPUBLIC_SITE_URLへの依存をなくし、エラーを解消します。
	const boards = [
		`<A HREF="/boards/1/">紅茶</A>`,
		`<A HREF="/boards/2/">なんU</A>`
	];

	// bbsmenu.htmlの形式でHTMLを構築します。
	const body = boards.join('<BR>\n');
	const html = `<HTML><HEAD><TITLE>BBS MENU</TITLE></HEAD><BODY>
<B>☕</B><BR>
${body}
</BODY></HTML>`;

	return createSjisResponse(html, 'text/html');
};
