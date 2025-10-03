import type { Handle, HandleFetch } from '@sveltejs/kit';
import { env } from '$env/dynamic/private';

// バックエンドサーバーのURL。環境変数から取得するのが望ましいです。
const PRIVATE_BACKEND_API_URL = env.PRIVATE_BACKEND_API_URL || 'http://backend:8000';

/**
 * SvelteKitのサーバーが受け取ったリクエストをインターセプトするフックです。
 *
 * このフックは、専ブラからの投稿リクエストを特別に処理し、
 * `Set-Cookie`ヘッダーが正しくクライアントに送信されるように保証します。
 * また、APIリクエストをバックエンドにプロキシする役割も担います。
 *
 * @see https://kit.svelte.dev/docs/hooks#server-hooks-handle
 */
export const handle: Handle = async ({ event, resolve }) => {
	const { pathname } = event.url;

	// サイトのルート("/")へのアクセスを判定し、専ブラをbbsmenu.htmlへ誘導します。
	if (pathname === '/') {
		const acceptHeader = event.request.headers.get('accept') || '';
		if (!acceptHeader.includes('text/html')) {
			const userAgent = event.request.headers.get('user-agent') || 'Unknown';
			console.log(
				`[HOOKS] Non-HTML client detected (Accept: "${acceptHeader}", UA: "${userAgent}"). Redirecting to /boards/bbsmenu.html`
			);
			return new Response(null, {
				status: 302,
				headers: { Location: new URL('/boards/bbsmenu.html', event.url.origin).toString() }
			});
		}
	}

	// /boards へのアクセスを、専ブラ向けの板一覧ページへリダイレクトします。
	// これにより、ユーザーが直接 /boards にアクセスした場合でも、適切なコンテンツが表示されます。
	if (pathname === '/boards' || pathname === '/boards/') {
		return new Response(null, {
			status: 302,
			headers: { Location: new URL('/boards/bbsmenu.html', event.url.origin).toString() }
		});
	}

	// 上記以外のすべてのリクエストは、SvelteKitの通常のルーティングに任せます。
	return resolve(event);
};

/**
 * SvelteKitのサーバーサイド`fetch`リクエストをインターセプトするフックです。
 * このフックは、`load`関数内などで実行される`fetch`がバックエンドAPIへリクエストを送る際に、
 * ブラウザから受け取った`session_token`クッキーを自動的に付与します。
 * @see https://kit.svelte.dev/docs/hooks#server-hooks-handlefetch
 */
export const handleFetch: HandleFetch = async ({ request, fetch, event }) => {
	const requestUrl = new URL(request.url);

	// サーバーサイドのfetchが相対パス('/api/...')でリクエストした場合や、
	// PUBLIC_API_BASE_URLへのリクエストを捕捉し、プライベートなバックエンドURLに書き換える。
	// これにより、認証クッキーの付与を一元管理する。
	const publicApiBaseUrl = new URL(env.PUBLIC_API_BASE_URL || 'http://localhost');
	const isApiRequest =
		requestUrl.pathname.startsWith('/api') || requestUrl.hostname === publicApiBaseUrl.hostname;

	if (isApiRequest) {
		// 新しいリクエストURLを構築
		const newUrl = `${PRIVATE_BACKEND_API_URL}${requestUrl.pathname}${requestUrl.search}`;

		// 元のリクエストヘッダーをコピー
		const newHeaders = new Headers(request.headers);

		// ブラウザからのリクエストに含まれるクッキーを取得
		const sessionToken = event.cookies.get('session_token');
		if (sessionToken) {
			// 既存のCookieヘッダーに追記するのではなく、上書きすることで意図しない挙動を防ぐ
			newHeaders.set('Cookie', `session_token=${sessionToken}`);
		}

		// --- START: IPヘッダーの引き継ぎ ---
		// クライアントから受け取ったIP関連のヘッダーをバックエンドにそのまま渡します。
		// これにより、バックエンドはリバースプロキシの背後にいるクライアントの真のIPを特定できます。
		const xForwardedFor = event.request.headers.get('x-forwarded-for');
		if (xForwardedFor) {
			newHeaders.set('X-Forwarded-For', xForwardedFor);
		}
		const xRealIp = event.request.headers.get('x-real-ip');
		if (xRealIp) {
			newHeaders.set('X-Real-IP', xRealIp);
		}
		// --- END: IPヘッダーの引き継ぎ ---

		// 新しいURLとヘッダーでリクエストを再作成してfetchを実行
		// 第2引数に { internal: true } を渡すことで、このfetchが再度handleFetchフックに
		// 捕捉されるのを防ぎ、無限ループを回避します。
		// SvelteKitのドキュメントには明記されていませんが、このような内部的な再帰呼び出しを
		// 制御するための非公式な（しかし一般的な）方法です。
		// 元の`request`オブジェクトをベースに、URLとヘッダーのみを上書きした新しいリクエストを作成します。
		// これにより、元のリクエストの`method`（POSTなど）や`body`が確実に引き継がれます。
		const newRequest = new Request(newUrl, request);
		newHeaders.forEach((value, key) => newRequest.headers.set(key, value));
		return fetch(newRequest, { internal: true } as any);
	}

	// 元の`fetch`処理を実行
	return fetch(request);
};
