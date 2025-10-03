import type { RequestHandler } from './$types';
import { PUBLIC_API_BASE_URL } from '$env/static/public';
import type { Board } from '$lib/types';
import { createSjisResponse } from '$lib/server/monacoin';

/**
 * バックエンドAPIの /api/boards/{id} から返されるレスポンスの型定義。
 */
interface BoardDetailResponse {
	board: Board;
}

/**
 * 専ブラが板のヘッダー情報 (head.txt) を読み込むためのエンドポイントです。
 * このファイルの内容は、通常、スレッド一覧の上部に表示されます。
 * ここでは、バックエンドから取得した板の「説明」を返します。
 */
export const GET: RequestHandler = async ({ params, fetch, setHeaders }) => {
	const { boardId } = params;

	try {
		// head.txtは頻繁にアクセスされる可能性があるため、キャッシュを設定します。
		// 404の場合もキャッシュすることで、存在しない板への頻繁なアクセスを抑制します。
		setHeaders({
			'Cache-Control': 'public, max-age=300, s-maxage=300' // 5分キャッシュ
		});

		const boardRes = await fetch(`${PUBLIC_API_BASE_URL}/boards/${boardId}`);

		// APIからのレスポンスが正常でない場合 (404 Not Found を含む)、
		// 専ブラがエラーを起こさないよう、空のhead.txtを返します。
		if (!boardRes.ok) {
			console.error(
				`API request for head.txt failed (board: ${boardId}, status: ${boardRes.status}). Returning empty head.txt.`
			);
			return createSjisResponse('', 'text/html', 200);
		}

		const responseData: BoardDetailResponse = await boardRes.json();
		const board = responseData.board;

		// 板の説明(description)をShift_JISでエンコードして返します。
		// Content-Typeは 'text/html' が一般的です。
		return createSjisResponse(board.description || '', 'text/html');
	} catch (e) {
		// fetch自体の失敗やJSONパースエラーなどを捕捉した場合も、
		// 同様に空のhead.txtを返して、専ブラのクラッシュを防ぎます。
		console.error(`Critical error generating head.txt for board ${boardId}:`, e);
		return createSjisResponse('', 'text/html', 200);
	}
};

