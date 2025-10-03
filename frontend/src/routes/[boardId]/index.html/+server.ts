import type { RequestHandler } from './$types';
import { PUBLIC_API_BASE_URL } from '$env/static/public';
import type { Post } from '$lib/types';
import { createSjisResponse } from '$lib/server/monacoin';
import { format } from 'date-fns';
import { ja } from 'date-fns/locale';

/**
 * スレッド一覧を人間が読みやすいHTML形式で提供するエンドポイントです。
 * 専ブラは通常 subject.txt を見ますが、Webブラウザからの直接アクセスを想定しています。
 */
export const GET: RequestHandler = async ({ params, fetch, setHeaders }) => {
	const { boardId } = params;

	try {
		// スレッド一覧は頻繁に更新されるため、キャッシュは短めに設定します。
		setHeaders({
			'Cache-Control': 'public, max-age=60, s-maxage=60' // 1分キャッシュ
		});

		// バックエンドAPIからスレッド一覧を取得
		const postsRes = await fetch(`${PUBLIC_API_BASE_URL}/boards/${boardId}/posts`);

		if (!postsRes.ok) {
			// APIエラーの場合は、空のページまたはエラーメッセージを表示
			console.error(
				`API request for index.html failed (board: ${boardId}, status: ${postsRes.status}).`
			);
			const errorHtml = `
<html>
<head><title>エラー</title></head>
<body>板情報の取得に失敗しました。 (Code: ${postsRes.status})</body>
</html>`;
			return createSjisResponse(errorHtml, 'text/html', postsRes.status);
		}

		const posts: Post[] = await postsRes.json();

		// スレッド一覧をHTMLのリストアイテムに変換
		const threadListItems = posts
			.map((post) => {
				const postDate = new Date(post.created_at);
				// 日付を 'yyyy/MM/dd HH:mm' 形式にフォーマット
				const formattedDate = format(postDate, 'yyyy/MM/dd HH:mm', { locale: ja });
				// スレッドへのリンクを生成 (datファイル名を使用)
				const timestamp = Math.floor(postDate.getTime() / 1000);
				const threadUrl = `../test/read.cgi/${boardId}/${timestamp}/`;
				const responseCount = post.response_count ?? 1;

				return `<li>
    <a href="${threadUrl}">${post.title}</a> (${responseCount})
    <small> - ${formattedDate}</small>
</li>`;
			})
			.join('\n');

		// 完全なHTMLページを構築
		const html = `
<!DOCTYPE html>
<html lang="ja">
<head>
    <meta charset="Shift_JIS">
    <title>スレッド一覧</title>
    <style>
        body { font-family: 'MS PGothic', 'Osaka', sans-serif; line-height: 1.6; }
        ul { list-style-type: none; padding-left: 0; }
        li { margin-bottom: 8px; }
        a { text-decoration: none; }
        a:hover { text-decoration: underline; }
        small { color: #555; }
    </style>
</head>
<body>
    <h1>スレッド一覧</h1>
    <ul>
        ${threadListItems}
    </ul>
</body>
</html>`;

		return createSjisResponse(html, 'text/html');
	} catch (e) {
		console.error(`Critical error generating index.html for board ${boardId}:`, e);
		return createSjisResponse('サーバーエラーが発生しました。', 'text/html', 500);
	}
};

