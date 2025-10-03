import type { RequestHandler } from './$types';
import type { Post } from '$lib/types';
import { PUBLIC_API_BASE_URL } from '$env/static/public';
import * as setCookieParser from 'set-cookie-parser';
import iconv from 'iconv-lite';
import { createHash } from 'crypto';
import * as https from 'https';
import he from 'he';
import * as http from 'http';
import { URL } from 'url';
import * as querystring from 'querystring';
type ApiResponse = {
	statusCode: number;
	headers: http.IncomingHttpHeaders;
	setCookieHeader?: string | string[]; // Set-Cookieヘッダーを隔離するための専用フィールド
	body: string;
};

/**
 * Node.jsのネイティブ`https`モジュールを使用してHTTPリクエストを送信するヘルパー関数。
 * axiosなどの高レベルライブラリとSvelteKitアダプタ間の競合を完全に回避するために使用します。
 */
function nativeHttpRequest(
	urlString: string, 
	options: https.RequestOptions, 
	payload?: string | Buffer
): Promise<ApiResponse> {
	return new Promise((resolve, reject) => {
		const url = new URL(urlString);
		// URLのプロトコルに応じて、httpモジュールとhttpsモジュールを動的に切り替えます。
		const protocol = url.protocol === 'https:' ? https : http;

		// payloadの型に応じてContent-Lengthをヘッダーに設定
		if (payload) {
			options.headers = {
				...options.headers,
				'Content-Length': Buffer.byteLength(payload)
			};
		}

		const req = protocol.request(url, options, (res) => {
			const chunks: Buffer[] = [];
			res.on('data', (chunk) => chunks.push(chunk));
			res.on('end', () => {
				const originalHeaders = res.headers;
				// バックエンドから受け取ったSet-Cookieヘッダーを退避させます。
				const setCookieHeader = originalHeaders['set-cookie'];

				// SvelteKitアダプタから隠すため、通常のheadersオブジェクトからは削除します。
				delete originalHeaders['set-cookie'];

				resolve({
					statusCode: res.statusCode || 500,
					headers: originalHeaders,
					setCookieHeader: setCookieHeader,
					body: Buffer.concat(chunks).toString('utf-8') // 最終的にUTF-8文字列に変換
				});
			});
		});
		req.on('error', reject);
		if (payload) {
			if (Buffer.isBuffer(payload)) {
				// ペイロードがBufferの場合、エンコーディングを指定せずに直接書き込みます。
				req.write(payload);
			} else {
				// ペイロードが文字列の場合、UTF-8としてエンコードして書き込みます。
				// このパスは現在使用されていませんが、将来の利用のために残しておきます。
				req.write(payload, 'utf-8');
			}
		}
		req.end();
	});
}

/**
 * 専ブラからの投稿リクエストを処理するCGIエンドポイント。
 * Shift_JISでエンコードされたフォームデータを受け取り、
 * スレッド作成またはレス作成のAPIを呼び出します。
 */
export const POST: RequestHandler = async ({ request, getClientAddress, cookies, url, locals }) => {
	let boardIdFromForm = ''; // catchブロックで使えるように外で宣言
	// --- START: ログ表示用のIPアドレス取得ロジックを修正 ---
	// Nginxから渡された `x-forwarded-for` ヘッダーを優先的に使用し、
	// 存在しない場合のみ、直接の接続元IPにフォールバックします。
	const xff = request.headers.get('x-forwarded-for');
	const clientIpForLogging = xff ? xff.split(',')[0].trim() : getClientAddress();
	const logPrefix = `[bbs.cgi] [IP: ${clientIpForLogging}]`;
	// --- END: ログ表示用のIPアドレス取得ロジックを修正 ---

	try {
		console.log(`${logPrefix} Received post request.`);

		// 専ブラから送られてきたCookieをバックエンドAPIに転送するための共通ヘッダーを準備します。
		// これにより、2回目以降の投稿でCookie認証が利用可能になります。
		const baseHeaders: http.OutgoingHttpHeaders = {};
		// Nginxから受け取ったIP関連ヘッダーをそのままバックエンドに渡します。
		const xForwardedFor = request.headers.get('x-forwarded-for');
		if (xForwardedFor) {
			baseHeaders['X-Forwarded-For'] = xForwardedFor;
		}
		const xRealIp = request.headers.get('x-real-ip');
		if (xRealIp) {
			baseHeaders['X-Real-IP'] = xRealIp;
		}
		// User-Agentも同様に引き継ぎます。
		// これにより、バックエンドは診断ログやフォールバックとして正しいUAを認識できます。
		const userAgentHeader = request.headers.get('user-agent');
		if (userAgentHeader) {
			baseHeaders['User-Agent'] = userAgentHeader;
		}	
		const incomingCookies = cookies.getAll();
		if (incomingCookies.length > 0) {
			const cookieHeader = incomingCookies.map((c) => `${c.name}=${c.value}`).join('; ');
			baseHeaders['Cookie'] = cookieHeader;
			console.log(`${logPrefix} Forwarding cookies to backend.`);
		}

		// 専ブラからのリクエスト情報から、バックエンドAPIに渡すためのデバイスIDを生成します。
		// Webブラウザからの投稿と一貫したID生成・レートリミットを適用するために重要です。
		const userAgent = request.headers.get('user-agent') || 'unknown';
		// 専ブラの場合は、User-Agent文字列そのものをデバイス情報(fingerprint)としてバックエンドに渡します。
		// これにより、バックエンドは生のUA文字列を元に永続IDを生成でき、管理画面での可読性も向上します。
		const fingerprint = userAgent;
		
		// 専ブラからのShift_JISリクエストボディをバイナリとして直接読み込みます。
		const buffer = Buffer.from(await request.arrayBuffer());
		
		// --- START: デバッグコード ---
		console.log(`[+server.ts DEBUG] Received raw buffer length: ${buffer.length}`);
		// [デバッグ 0] 受け取った一番最初の生データ(Buffer)を16進数で出力
		console.log('\n--- [DEBUG 0: Raw Buffer Content (Hex)] ---');
		console.log(buffer.toString('hex'));
		console.log('-------------------------------------------\n');
		// --- END: デバッグコード ---
		
		// querystring.parseはBufferを直接受け取ることができます。
		// Bufferを渡した場合、decodeURIComponentはキーと値の両方に適用されます。
		// これにより、キー名（例: 'submit'）と値（例: '%8f%91%82%ab%8d%9e%82%de'）の両方を
		// 正しくデコードできます。
		const decodedFields = querystring.parse(
			// bufferを文字列に変換する際、'ascii'エンコーディングは7ビットしか扱えず、
			// Shift_JISのパーセントエンコーディング（例: %8f）のような8ビット文字を破壊してしまいます。
			// 'latin1'は各バイトをそのまま文字コードにマッピングするため、データが破壊されません。
			buffer.toString('latin1'),
			'&',
			'=',
			{
				decodeURIComponent: (str) => {
					// 専ブラからのデータは、Shift_JISのパーセントエンコーディングと、
					// 未エンコードの文字（英数字や、Shift_JISにない絵文字など）が混在します。
					// このような文字列を堅牢にデコードするための新しいロジックです。
					const bytes: number[] = [];
					const buffer = Buffer.from(str, 'latin1');
					let i = 0;
					while (i < buffer.length) {
						if (buffer[i] === 0x25 /* % */ && i + 2 < buffer.length) {
							const hex = buffer.toString('ascii', i + 1, i + 3);
							const byte = parseInt(hex, 16);
							if (!isNaN(byte)) {
								bytes.push(byte);
								i += 3;
								continue;
							}
						}
						// パーセントエンコーディングでないバイト、または不正な形式の場合は、
						// そのバイトをそのままbytesに追加します。
						bytes.push(buffer[i]);
						i++;
					}

					const sjisCandidateBuffer = Buffer.from(bytes);
					let decodedStr = '';
					let lastDecodedIndex = 0;

					// iconv.decodeStream() を模倣し、デコード可能な部分をShift_JISとしてデコードし、
					// デコードできない部分（例: 絵文字など）は元のバイトシーケンス（UTF-8文字として解釈）のまま残します。
					for (let i = 0; i <= buffer.length; i++) {
						try {
							// iバイト目までをShift_JISとしてデコード試行
							const decodedPart = iconv.decode(sjisCandidateBuffer.slice(0, i), 'shift_jis');
							// 成功したら、その部分を確定し、次の開始位置を更新
							decodedStr = decodedPart;
							lastDecodedIndex = i;
						} catch (e) {
							// デコードに失敗した場合、そのバイトはShift_JISの文字の一部ではない。
							// 最後の成功地点から現在位置までの未デコード部分を元の文字として追加し、
							// lastDecodedIndexを更新して処理を続行する。(try-catchはパフォーマンスに影響するため、頻繁な失敗は避けるべき)
							if (i > lastDecodedIndex) {
								decodedStr += buffer.slice(lastDecodedIndex, i).toString('utf-8');
								lastDecodedIndex = i;
							}
						}
					}
					str = decodedStr;

					// Shift_JISデコード後、または元々パーセントエンコーディングされていなかった文字列に対し、
					// HTMLエンティティ（例: &gt;, &amp;, &#12345;）をデコードします。
					return he.decode(str);
				}
			}
		);

		// --- START: デバッグコード ---
		// [デバッグ 3] querystring.parseで完全にデコードされた後のオブジェクト。
		console.log('\n--- [DEBUG 3: querystring.parse result] ---');
		console.log(decodedFields);
		console.log('-------------------------------------------\n');
		// --- END: デバッグコード ---
		
		// 後続の処理で使いやすいようにURLSearchParamsに変換します。
		const formData = new URLSearchParams(decodedFields as Record<string, string>);
		
		// フォームデータから各値を取得
		boardIdFromForm = formData.get('bbs') || '';
		const threadKey = formData.get('key') || '';
		const body = formData.get('MESSAGE') || '';
		const authorName = formData.get('FROM') || '';
		const subject = formData.get('subject') || '';

		console.log(
			`${logPrefix} Parsed form data. Board: ${boardIdFromForm}, Subject: ${subject ? 'Yes' : 'No'}, Key: ${
				threadKey || 'N/A'
			}`
		);

		if (!boardIdFromForm || !body) {
			console.error(`${logPrefix} Validation failed: Missing boardId or body.`);
			return createCgiErrorResponse('板IDまたは本文がありません。', boardIdFromForm, null, cookies, url.origin);
		}

		// subjectがあればスレッド作成、なければレス作成 (エラーハンドリングを改善)
		if (subject) {
			// --- スレッド作成処理 ---
			const payload = {
				board_id: Number(boardIdFromForm),
				title: subject,
				body: body,
				author_name: authorName || undefined,
				fingerprint: fingerprint
			};

			// --- START: 診断コード ---
			// バックエンドに送信する直前のペイロードオブジェクトをログに出力
			console.log('\n--- [DEBUG 4: Payload to Backend (Thread)] ---');
			console.log(payload);
			console.log('---------------------------------------------\n');
			// --- END: 診断コード ---

			// ペイロードをUTF-8のBufferに変換します。これによりContent-Lengthが正確に計算され、文字化けを防ぎます。
			const payloadBuffer = Buffer.from(JSON.stringify(payload), 'utf-8');

			const options: https.RequestOptions = {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json; charset=utf-8',
					...baseHeaders,
				}
			};
			const apiResponse = await nativeHttpRequest(`${PUBLIC_API_BASE_URL}/posts`, options, payloadBuffer);

			if (apiResponse.statusCode >= 400) {
				return createCgiErrorResponse('スレッドの作成に失敗しました。', boardIdFromForm, apiResponse, cookies, url.origin);
			}

			const createdPost: Post = JSON.parse(apiResponse.body);
			return createCgiSuccessResponse('スレッドを作成しました。', boardIdFromForm, String(createdPost.id), apiResponse, cookies);
		} else {
			// --- レス作成処理 ---
			if (!threadKey) {
				console.error(`${logPrefix} Validation failed: Missing thread key for comment.`);
				return createCgiErrorResponse('スレッドが指定されていません。', boardIdFromForm, null, cookies, url.origin);
			}
			// 専ブラはスレッドのタイムスタンプ(datファイル名)を 'key' として送ってきます。
			// これをバックエンドAPIが要求する 'post_id' に変換する必要があります。
			// タイムスタンプから直接スレッドを検索するAPIを呼び出します。
			// これにより、板の全スレッドを取得する必要がなくなり、パフォーマンスが大幅に向上します。
			const getUrl = `${PUBLIC_API_BASE_URL}/posts/by-timestamp/${threadKey}?board_id=${boardIdFromForm}`;
			const getOptions: https.RequestOptions = {
				method: 'GET',
				headers: baseHeaders
			};
			const postRes = await nativeHttpRequest(getUrl, getOptions);

			if (postRes.statusCode >= 400) {
				const defaultMessage =
					postRes.statusCode === 404 ? '該当のスレッドが見つかりません。' : 'スレッド情報の取得に失敗しました。';
				return createCgiErrorResponse(defaultMessage, boardIdFromForm, postRes, cookies, url.origin);
			}
			const targetPost: Post = JSON.parse(postRes.body);

			const payload = {
				post_id: targetPost.id,
				body: body,
				author_name: authorName || undefined,
				fingerprint: fingerprint
			};

			// --- START: 診断コード ---
			// バックエンドに送信する直前のペイロードオブジェクトをログに出力
			console.log('\n--- [DEBUG 4: Payload to Backend (Comment)] ---');
			console.log(payload);
			console.log('---------------------------------------------\n');
			// --- END: 診断コード ---

			// ペイロードをUTF-8のBufferに変換します。これによりContent-Lengthが正確に計算され、文字化けを防ぎます。
			const payloadBuffer = Buffer.from(JSON.stringify(payload), 'utf-8');

			const postCommentOptions: https.RequestOptions = {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json; charset=utf-8',
					...baseHeaders,
				}
			};
			const apiResponse = await nativeHttpRequest(`${PUBLIC_API_BASE_URL}/comments`, postCommentOptions, payloadBuffer);

			if (apiResponse.statusCode >= 400) {
				return createCgiErrorResponse('レスの投稿に失敗しました。', boardIdFromForm, apiResponse, cookies, url.origin);
			}

			return createCgiSuccessResponse('レスを投稿しました。', boardIdFromForm, threadKey, apiResponse, cookies);
		}
	} catch (e) {
		const errorMessage = e instanceof Error ? e.message : String(e);
		console.error(`${logPrefix} An unexpected error occurred: ${errorMessage}`, e);
		return createCgiErrorResponse('サーバーで予期せぬエラーが発生しました。', boardIdFromForm, null, cookies, url.origin);
	}
};

/**
 * GETリクエストは専ブラでは通常使用されませんが、
 * デバッグや将来の拡張のために空のレスポンスを返すように実装しておきます。
 */
export const GET: RequestHandler = async () => {
	const responseHtml = `
	<!DOCTYPE HTML PUBLIC "-//W3C//DTD HTML 4.01 Transitional//EN">
	<html><head><title>Unsupported Method</title></head>
	<body>GET method is not supported for this endpoint.</body></html>
	`;
	const body = iconv.encode(responseHtml, 'Shift_JIS');
	const headers = new Headers({ 'Content-Type': 'text/html; charset=Shift_JIS' });
	return new Response(body, { status: 405, headers });
};

/**
 * 専ブラ向けの投稿成功HTMLを生成して返すヘルパー関数
 */
function createCgiSuccessResponse(
	message: string,
	boardId: string,
	threadKey: string,
	apiResponse: ApiResponse,
	cookies: import('@sveltejs/kit').Cookies
): Response {
	console.log(
		`[bbs.cgi] Successfully processed request for board ${boardId}. Message: "${message}", Status: ${apiResponse.statusCode}`
	);

	// 専ブラがリダイレクトするためのURLを構築します。
	// 例: ../read.cgi/boardId/threadKey/
	const redirectUrl = `../read.cgi/${boardId}/${threadKey}/`;

	const responseHtml = `
<!DOCTYPE HTML PUBLIC "-//W3C//DTD HTML 4.01 Transitional//EN">
<html>
<head>
<title>書きこみました。</title>
<meta http-equiv="refresh" content="2;URL=${redirectUrl}">
</head>
<body>
書きこみました。<br><br>
<a href="${redirectUrl}" target="_top">画面を切り替える</a>
</body>
</html>
`;

	// バックエンドから受け取ったSet-Cookieヘッダーをパースし、SvelteKitのcookies APIで設定します。
	// これにより、SvelteKitが正しいSet-Cookieヘッダーをレスポンスに付与します。
	const setCookieHeader = apiResponse.setCookieHeader;
	if (setCookieHeader) {
		const parsedCookies = setCookieParser.parse(setCookieHeader);
		parsedCookies.forEach((cookie) => {
			cookies.set(cookie.name, cookie.value, {
				path: cookie.path,
				expires: cookie.expires,
				httpOnly: cookie.httpOnly,
				secure: cookie.secure,
				sameSite: cookie.sameSite?.toLowerCase() as 'lax' | 'strict' | 'none' | undefined
			});
		});
	}

	const body = iconv.encode(responseHtml, 'Shift_JIS');
	const headers = new Headers({
		'Content-Type': 'text/html; charset=Shift_JIS'
	});

	return new Response(body, { status: 200, headers: headers });
}

/**
 * 専ブラ向けの投稿失敗HTMLを生成して返すヘルパー関数
 */
function createCgiErrorResponse(
	defaultMessage: string,
	boardId: string,
	apiResponse: ApiResponse | null,
	cookies: import('@sveltejs/kit').Cookies,
	origin: string
): Response {
	let errorMessage = defaultMessage;
	let statusCode = 400; // デフォルトは 400 Bad Request

	if (apiResponse) {
		statusCode = apiResponse.statusCode;
		// APIからのエラーレスポンスボディをパースして、より具体的なエラーメッセージを取得します。
		try {
			// ボディが空、またはJSONでない可能性を考慮
			if (apiResponse.body) {
				const errorJson = JSON.parse(apiResponse.body);
				// バックエンドが { "error": "メッセージ" } という形式で返すことを期待
				errorMessage = errorJson.error || defaultMessage;
			}
		} catch (parseError) {
			// JSONパースに失敗した場合 (HTMLエラーページなどが返ってきた場合) は、ボディをそのままメッセージとします。
			errorMessage = apiResponse.body || defaultMessage;
			console.error(
				`[bbs.cgi] Failed to parse API error response for board ${boardId}. Status: ${statusCode}. Body:`,
				apiResponse.body
			);
		}
	}

	// 認証エラー(401)の場合、ユーザーに具体的なアクションを促すメッセージに上書きします。
	if (statusCode === 401) {
		const authUrl = `${origin}/auth/register`;
		errorMessage = `投稿するためには、<a href="${authUrl}" target="_blank">${authUrl}</a> にて認証して、専ブラ連携トークンを取得してください`;
	}

	console.error(
		`[bbs.cgi] Responding with error for board ${boardId}. Status: ${statusCode}, Message: "${errorMessage}"`
	);

	const responseHtml = `
<!DOCTYPE HTML PUBLIC "-//W3C//DTD HTML 4.01 Transitional//EN">
<html>
<head>
<title>ERROR!</title>
</head>
<body>
ERROR: ${errorMessage}<br><br>
</body>
</html>
`;

	// エラー時でもCookieが発行される場合があるため、同様にヘッダーを設定します。
	if (apiResponse && apiResponse.setCookieHeader) {
		const parsedCookies = setCookieParser.parse(apiResponse.setCookieHeader);
		parsedCookies.forEach((cookie) => {
			cookies.set(cookie.name, cookie.value, {
				path: cookie.path,
				expires: cookie.expires,
				httpOnly: cookie.httpOnly,
				secure: cookie.secure,
				sameSite: cookie.sameSite?.toLowerCase() as 'lax' | 'strict' | 'none' | undefined
			});
		});
	}

	const body = iconv.encode(responseHtml, 'Shift_JIS');
	const headers = new Headers({
		'Content-Type': 'text/html; charset=Shift_JIS'
	});

	return new Response(body, { status: statusCode, headers: headers });
}
