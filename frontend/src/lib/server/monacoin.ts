import iconv from 'iconv-lite';
import type { Board, Post, Comment } from '../../app';
import { format } from 'date-fns';
import { ja } from 'date-fns/locale';
import { getLevelDisplayString } from '$lib/levelDisplay';

/**
 * Shift_JISにエンコードされたResponseオブジェクトを生成します。
 * @param text - レスポンスボディとなるテキスト
 * @param contentType - Content-Typeヘッダーの値
 * @param status - HTTPステータスコード (デフォルトは200)
 * @returns Shift_JISエンコードされたResponseオブジェクト
 */
export function createSjisResponse(
	text: string,
	contentType: string,
	status = 200,
	setCookieHeaders: string[] = []
): Response {
	const encodedBody = iconv.encode(text, 'Shift_JIS');
	// 古い専ブラはチャンク分割転送(Transfer-Encoding: chunked)を正しく解釈できない場合があるため、
	// レスポンスの総バイト数を Content-Length ヘッダーで明示的に指定します。
	// これにより、サーバーはレスポンスをチャンクせず、一体のデータとして送信するようになります。
	const contentLength = encodedBody.length.toString();
	const headers = new Headers();
	headers.set('Content-Type', `${contentType}; charset=Shift_JIS`);
	headers.set('Content-Length', contentLength);

	// 受け取ったSet-Cookieヘッダーを個別に、かつ安全にHeadersオブジェクトに追加します。
	for (const cookie of setCookieHeaders) {
		headers.append('set-cookie', cookie);
	}

	return new Response(encodedBody, {
		status,
		headers
	});
}

/**
 * 板一覧からbbsmenu.html形式のHTMLを生成します。
 * @param boards - 板情報の配列
 * @returns bbsmenu.html形式のHTML文字列
 */
export function generateBbsmenuHtml(boards: Board[]): string {
	// 専ブラが板のURLを正しく認識できるよう、板のルートディレクトリへのリンクを生成します。
	// 例: <A HREF="/news/">ニュース</A>
	// これにより、ブラウザは板のベースURLが /news/ であると解釈し、subject.txt を正しく見つけます。
	const boardLinks = boards
		.map((board) => `<A HREF="/boards/${board.id}/">${escapeDatField(board.name)}</A>`)
		.join('<br>\n');

	// カテゴリ分けが必要な場合は、ここでロジックを追加します
	const body = `<B>カテゴリ</B><br>\n${boardLinks}`;

	return `<HTML><HEAD><TITLE>BBS MENU</TITLE></HEAD><BODY>${body}</BODY></HTML>`;
}

/**
 * スレッド一覧からsubject.txt形式のテキストを生成します。
 * @param posts - スレッド情報の配列
 * @returns subject.txt形式のテキスト文字列
 */
export function generateSubjectTxt(posts: Post[]): string {
	return (
		posts
			// APIからidやtitleが欠落した不正なデータが返ってきた場合に備え、それらをフィルタリングで除外します。
			// これにより、一部のデータに問題があってもsubject.txt全体の形式が壊れるのを防ぎます。
			.filter((post) => post && typeof post.id === 'number' && typeof post.title === 'string')
			.map((post) => {
				// 専ブラが作成日時を正しく表示できるよう、ファイル名にはUnixタイムスタンプ(秒)を使用します。
				// APIから渡されるcreated_atはUTCのISO文字列。new Date()でパースすると、実行環境のタイムゾーンのDateオブジェクトになるため、
				// これをそのままUnixタイムスタンプに変換すれば、正しい時刻基点となる。
				const timestamp = Math.floor(new Date(post.created_at).getTime() / 1000);

				// 2ch形式の「レス数」はスレッド本体(1レス目)を含んだ総数です。
				// バックエンドAPIが返す `response_count` が既にこの総数を表しているため、不要な `+1` を削除します。
				// `response_count` が未定義の場合、スレッド本体の1レスは必ず存在するため、フォールバックとして `1` を使用します。
				const responseCount = post.response_count ?? 1;
				return `${timestamp}.dat<>${escapeDatField(post.title)} (${responseCount})`;
			})
			.join('\n') + '\n'
	);
}

/**
 * 板情報からSETTING.TXT形式のテキストを生成します。
 * このファイルは、専ブラが板の名前やデフォルト名無しさんなどの設定を読み込むために使用します。
 * @param board - 板情報
 * @returns SETTING.TXT形式のテキスト文字列
 */
export function generateSettingTxt(board: Board): string {
	const settings = {
		// 板名は必須。APIから取得できなかったり空だったりした場合、専ブラが板IDを表示してしまう問題を防ぐため、
		// || 演算子で堅牢なフォールバックを設定します。
		'BBS_TITLE': board.name || `名称未設定の板: ${board.id}`,

		// 多くの専ブラは空のBBS_NONAME_NAME (`BBS_NONAME_NAME=`) をエラーと見なすため、
		// null, undefined, 空文字列のすべての場合に安全なデフォルト値「名無しさん」を設定します。
		// そのため、ここでは || 演算子が意図した動作となります。
		'BBS_NONAME_NAME': board.default_name || '野球民',

		'BBS_LINE_NUMBER': '20',
		'BBS_SUBJECT_COUNT': '400',
		'BBS_NAME_COUNT': '40',
		'BBS_MAIL_COUNT': '20',
		'BBS_MESSAGE_COUNT': '3000'
	};

	return Object.entries(settings)
		// SETTING.TXTの各値は、datファイルのように特殊文字をエスケープする必要はありません。
		// 誤ってescapeDatFieldを使用すると、必須である改行コードがスペースに置換され、
		// ファイル全体が1行になってしまい、専ブラが解析に失敗します。
		.map(([key, value]) => `${key}=${value}`)
		.join('\n') + '\n';
}

/**
 * APIからの板情報取得に失敗した際に、最低限のSETTING.TXTを生成するための
 * フォールバック用のBoardオブジェクトを作成します。
 * @param boardId - 板ID
 * @returns フォールバック用のBoardオブジェクト
 */
export function createFallbackBoard(boardId: string): Board {
	// generateSettingTxtが必要とする最低限のプロパティを持つオブジェクトを返す。
	// Board型の他のプロパティは、このコンテキストでは不要なため省略可能。
	return {
		id: parseInt(boardId, 10) || 0, // idはnumber型なので変換
		name: '（板が見つかりません）',
		description: 'この板は存在しないか、取得できませんでした。', // Board型に存在するプロパティ
		default_name: '野球民',
		created_by: null, // Board型に存在するプロパティ
		created_at: new Date().toISOString(),
		updated_at: new Date().toISOString(),
		deleted_at: null, // Board型に存在するプロパティ
		last_activity_at: new Date().toISOString(), // Board型に存在するプロパティ
		archived_at: null, // Board型に存在するプロパティ
		max_posts: 0, // Board型に存在するプロパティ
		// Board型で必須だがgenerateSettingTxtでは使用されないプロパティにデフォルト値を設定
		moderation_type: 'alpha',
		auto_archive_enabled: false,
		can_moderate: false,
	};
}

/**
 * dat形式のフィールドをエスケープします。
 * dat形式の区切り文字である <> との衝突を避けるため、基本的なHTMLエスケープを行います。
 * @param text エスケープする文字列
 * @returns エスケープされた文字列
 */
export function escapeDatField(text: string | null | undefined): string {
	if (!text) return '';
	// datの区切り文字である `<` との衝突を避けるためのエスケープ。
	// `>` はレスアンカー `>>` を破壊してしまう副作用があるため、エスケープ対象から除外します。
	// 改行コードもファイル形式を破壊するため、スペースに置換して無害化します。
	return text
		.replace(/&/g, '&amp;')
		.replace(/</g, '&lt;')
		.replace(/\r?\n/g, ' ');
}

/**
 * Dateオブジェクトを2ch互換のdatファイル形式の日付文字列に変換します。
 * @param date - 変換するDateオブジェクト
 * @returns "YYYY/MM/DD(曜) HH:mm:ss.ss" 形式の文字列
 */
function formatDatDate(utcDate: Date): string {
	// toLocaleStringを使ってJSTの各部分を取得
	const jstString = utcDate.toLocaleString('ja-JP', {
		timeZone: 'Asia/Tokyo',
		year: 'numeric',
		month: '2-digit',
		day: '2-digit',
		weekday: 'short',
		hour: '2-digit',
		minute: '2-digit',
		second: '2-digit',
		hour12: false
	});
	// "2024/07/27(土) 15:00:00" のような形式に整形
	// jstString は "2024/07/27(土) 15:00:00" のような形式で返ってくる
	const [datePart, timePart] = jstString.split(' '); // ["2024/07/27(土)", "15:00:00"]
	const [year, month, dayWithWeek] = datePart.split('/'); // ["2024", "07", "27(土)"]
	const [hours, minutes, seconds] = timePart.split(':'); // ["15", "00", "00"]
	const milliseconds = utcDate.getMilliseconds().toString().padStart(3, '0').slice(0, 2);
	return `${year}/${month}/${dayWithWeek} ${hours}:${minutes}:${seconds}.${milliseconds}`;
}

/**
 * 投稿またはコメントのデータをdat形式の一行にフォーマットします。
 * @param entity - PostまたはCommentオブジェクト
 * @param title - スレッドタイトル（1レス目のみ）
 * @returns dat形式の1行
 */
function formatDatLine(
	entity: Post | Comment,
	title?: string | null | undefined
): string {
	// 名無しの場合のフォールバックを設定
	let name = escapeDatField(entity.author_name || '名無しさん');
	// メール欄にレベル文字列を表示
	const mail = getLevelDisplayString(entity);
	const date = formatDatDate(new Date(entity.created_at));

	// --- START: IDの表示ロジック ---
	// display_user_id はバックエンドから "user-ip-device" の形式で渡されることを想定
	const idParts = (entity.display_user_id || '').split('-');
	let idForDate: string;

	if (idParts.length === 3) {
		// idParts[0] = user, idParts[1] = ip, idParts[2] = device
		// 名前の後ろに (IP-Device) を表示
		const idForName = ` (${idParts[1]}-${idParts[2]})`;
		name += idForName;
		// ID欄に User ID を表示
		idForDate = idParts[0];
	} else {
		// 予期せぬ形式の場合のフォールバック。display_user_id全体を名前に表示し、ID欄は'????'とする
		name += ` (${entity.display_user_id || '????'})`;
		idForDate = '????';
	}

	const dateAndId = `${date} ID:${idForDate}`;
	// --- END: IDの表示ロジック ---

	// バックエンドから渡される本文には、HTMLエンティティ化された<a>タグや<br>タグ (`&lt;a...&gt;`) と、
	// レスアンカー (`&gt;&gt;43`) が混在しています。
	// <a>と<br>タグのみをデコードし、レスアンカーはそのまま残すことで、専ブラでの表示を両立させます。
	let content = entity.body || '';
	// 1. &lt;a...&gt; や &lt;br&gt; のようなHTMLタグのみをデコードする
	content = content.replace(/&lt;(\/?(a|br)[^&]*)&gt;/g, '<$1>');
	// 2. その他のエスケープ文字(&amp;など)をデコードする
	content = content.replace(/&amp;/g, '&');
	// 3. 本文中の素の改行コード(\n)を<br>タグに変換する
	content = content.replace(/\r?\n/g, '<br>');
	// 4. 連続する<br>タグを1つにまとめる
	content = content.replace(/(<br\s*\/?>\s*){2,}/gi, '<br>');
	const escapedTitle = title ? escapeDatField(title) : '';

	return `${name}<>${mail}<>${dateAndId}<> ${content} <>${escapedTitle}`;
}

/**
 * スレッドとコメントからdat形式のテキストを生成します。
 * @param post - スレッド本体の情報
 * @param comments - コメントの配列
 * @returns dat形式のテキスト文字列
 */
export function generateDat(post: Post, comments: Comment[]): string {
	// 1レス目 (スレッド本体)
	const postLine = formatDatLine(post, post.title);
	// 2レス目以降
	const commentLines = comments.map((comment) => formatDatLine(comment));
	const lines = [postLine, ...commentLines];
	return lines.join('\n') + '\n';
}
