import type { RequestHandler } from './$types';
import { generateSubjectTxt, createSjisResponse } from '$lib/server/monacoin';
import { PUBLIC_API_BASE_URL } from '$env/static/public';
import type { Post } from '$lib/types';

export const GET: RequestHandler = async ({ params, fetch, setHeaders }) => {
	const { boardId } = params;
	try {
		const postsRes = await fetch(`${PUBLIC_API_BASE_URL}/boards/${boardId}/posts`);

		// subject.txtは頻繁にアクセスされるためキャッシュを設定し、負荷を軽減します。
		// 404の場合もキャッシュすることで、存在しない板への頻繁なアクセスを抑制します。
		setHeaders({
			'Cache-Control': 'public, max-age=60, s-maxage=60'
		});

		// APIからのレスポンスが正常でない場合 (404 Not Found を含む)、
		// 専ブラが「解析できませんでした」というエラーを起こさないよう、
		// 常に空のsubject.txtを返します。これにより、専ブラ側では「スレッドが0件の板」として
		// 正常に処理され、ユーザー体験を損ないません。
		if (!postsRes.ok) {
			// サーバーログにはエラーの詳細を記録して、デバッグに役立てます。
			console.error(`API request for subject.txt failed (board: ${boardId}, status: ${postsRes.status}). Returning empty subject.txt.`);
			return createSjisResponse('', 'text/plain', 200); // ステータスは200 OKを返すのが無難
		}

		// APIからの生のレスポンスボディをログに出力して、データ形式を確認します。
		// これにより、バックエンドが本当に空の配列を返しているのか、
		// それとも予期しない形式のデータを返しているのかを切り分けます。
		const responseText = await postsRes.text();
		console.log(`[DEBUG subject.txt] Raw API response for board ${boardId}:\n${responseText}`);

		// APIからのレスポンスは { "items": [...] } でラップされていない、
		// Postの配列そのものであるため、直接配列としてパースします。
		// これにより、APIからのデータが正しく解釈されます。
		const posts: Post[] = JSON.parse(responseText);

		// 【デバッグログ①】APIから取得したスレッドの数と内容（一部）を記録
		console.log(`[DEBUG subject.txt] Board ${boardId}: Fetched ${posts.length} posts.`);
		if (posts.length > 0) {
			console.log(`[DEBUG subject.txt] First post title: "${posts[0].title}", ID: ${posts[0].id}`);
		}

		const subjectText = generateSubjectTxt(posts);
		// 【デバッグログ②】生成したsubject.txtの最終的な内容を記録
		console.log(`[DEBUG subject.txt] Generated text for board ${boardId}:\n---BEGIN---\n${subjectText}---END---`);
		return createSjisResponse(subjectText, 'text/plain');
	} catch (e) {
		// fetch自体の失敗(ネットワークエラー)や、JSONパースエラーなどを捕捉した場合も、
		// 同様に空のsubject.txtを返して、専ブラのクラッシュを防ぎます。
		console.error(`Critical error generating subject.txt for board ${boardId}:`, e);
		return createSjisResponse('', 'text/plain', 200);
	}
};
