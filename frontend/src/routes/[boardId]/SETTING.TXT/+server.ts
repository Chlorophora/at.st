import type { RequestHandler } from './$types';
import { generateSettingTxt, createFallbackBoard, createSjisResponse } from '$lib/server/monacoin';
import { PUBLIC_API_BASE_URL } from '$env/static/public';
import type { Board } from '$lib/types';

// このエンドポイントは動的にSETTING.TXTを生成するため、ビルド時の事前レンダリング(prerendering)を無効化します。
// これを`true`にすると、ビルド時に生成された古い内容が返され続け、
// データベースの変更やコードの修正が反映されなくなります。
// `false`に設定することで、リクエストのたびにサーバーサイドのコードが実行されることが保証されます。
export const prerender = false;

/**
 * バックエンドAPIの /api/boards/{id} から返されるレスポンスの型定義。
 * Board情報に加えて、モデレーション権限などの追加情報が含まれます。
 */
interface BoardDetailResponse {
	board: Board & {
		can_moderate: boolean;
	};
	// creator_infoなどの他のフィールドも存在する可能性がありますが、
	// SETTING.TXTの生成には不要なため、ここでは省略します。
}

/**
 * 専ブラが板の設定を読み込むための SETTING.TXT を提供するエンドポイントです。
 */
export const GET: RequestHandler = async ({ params, fetch, setHeaders }) => {
	const { boardId } = params; // SvelteKitはファイル名からパラメータを推測するため、このままでも動作しますが、idに統一するのが望ましいです。

	try {
		// バックエンドから板の詳細情報を取得
		// SvelteKitのfetchが内部で結果をキャッシュすることがあるため、
		// `cache: 'no-store'` を明示的に指定し、リクエストのたびに必ず API を叩くように強制します。
		// これにより、コード変更やDB更新が即座に反映されるようになります。
		const boardRes = await fetch(`${PUBLIC_API_BASE_URL}/boards/${boardId}`, { cache: 'no-store' });

		// SETTING.TXTも頻繁にアクセスされるためキャッシュを設定し、負荷を軽減します。
		setHeaders({
			'Cache-Control': 'public, max-age=300, s-maxage=300' // 5分キャッシュ
		});

		let boardData: Board;

		if (!boardRes.ok) {
			console.error(
				`API request for SETTING.TXT failed (board: ${boardId}, status: ${boardRes.status}).`
			);
			// APIリクエストが失敗した場合は、フォールバック用のデータを使用します。
			boardData = createFallbackBoard(boardId);
		} else {
			// バックエンドからのレスポンスは { board: { ... } } という構造になっているため、
			// 正しく `board` プロパティを抽出します。また、デバッグのために生レスポンスもログに出力します。
			const responseText = await boardRes.text();
			console.log(`[DEBUG SETTING.TXT] Raw API response for board ${boardId}:\n${responseText}`);
			const responseData: BoardDetailResponse = JSON.parse(responseText);
			boardData = responseData.board;
		}

		// 【デバッグログ①】APIから取得した板のデータを記録
		console.log(`[DEBUG SETTING.TXT] Board data for ${boardId}:`, JSON.stringify(boardData, null, 2));

		const settingText = generateSettingTxt(boardData);

		// 【デバッグログ②】生成されたSETTING.TXTの生の内容を確認
		console.log(`[DEBUG SETTING.TXT] Generated raw SETTING.TXT for ${boardId}:\n---\n${settingText}---`);

		// subject.txtがLFで動作していることから、SETTING.TXTもLFで試します。
		// 専ブラによってはLFを許容する場合があるため、CRLFへの強制変換は一旦削除します。

		// createSjisResponseを使用して、Content-Lengthを含む適切なヘッダーを持つレスポンスを生成します。
		return createSjisResponse(settingText, 'text/plain');
	} catch (e) {
		console.error(`Critical error generating SETTING.TXT for board ${boardId}:`, e);
		// クリティカルエラー時も、専ブラがクラッシュしないように空のレスポンスを返します。
		return createSjisResponse('BBS_TITLE=サーバーエラー\n', 'text/plain', 500); // エラーメッセージもLFに統一
	}
};