import type { RequestHandler } from './$types';

/**
 * 古い形式の `subback.html` へのアクセスを、スレッド一覧である `subject.txt` へリダイレクトします。
 * 専ブラが `/boards/{id}/subback.html` のようなパスでアクセスしてきた場合も考慮し、
 * リダイレクト先を現在のパス構造に合わせることで、一貫性を保ちます。
 */
export const GET: RequestHandler = ({ url }) => {
	// 現在のパスから `subback.html` を `subject.txt` に置き換える
	const newPath = url.pathname.replace(/subback\.html$/, 'subject.txt');

	return new Response(null, {
		status: 302,
		headers: {
			Location: newPath
		}
	});
};
