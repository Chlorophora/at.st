<!--
    このファイルは、新しい認証フローをフロントエンドでどのように実装するかの具体例です。
    実際のプロジェクトの構造やUIに合わせて適宜修正してください。
-->
<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { fly } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { goto } from '$app/navigation';
	import { getRawFingerprintData } from '$lib/utils/fingerprint';
	import { PUBLIC_HCAPTCHA_SITE_KEY, PUBLIC_TURNSTILE_SITE_KEY } from '$env/static/public';

	// 診断ログ: スクリプトのトップレベルで環境変数の値を表示
	console.log(`[診断 0/5] スクリプト読み込み時点。PUBLIC_TURNSTILE_SITE_KEY:`, PUBLIC_TURNSTILE_SITE_KEY);

	// 状態をより詳細に管理
	type Step = 'agreement_and_challenges' | 'loading' | 'account_id_input' | 'account_id_display' | 'linking_token' | 'error';

	let step: Step = 'agreement_and_challenges'; // 初期状態は同意とチャレンジ
	// let email = ''; // メールアドレスは使用しないためコメントアウト
	let preflightToken = '';
	let accountId = '';
	let errorMessage = '';
	let infoMessage = '';
	let loading = false;
	let termsAgreed = false;

	let registrationContainer: HTMLDivElement;
	// --- Challenge関連の変数 ---
	let HCaptchaComponent: any = null; // HCaptchaコンポーネントを保持する変数
	// let TurnstileComponent: any = null; // svelte-turnstileライブラリは使用しない
	let turnstileInterval: any;
	let hCaptchaReady = false; // hCaptchaの準備完了フラグ
	let turnstileScriptReady = false; // Turnstileスクリプトの準備完了フラグ

	let hcaptchaToken: string | null = null;
	let turnstileToken: string | null = null;

	// --- Turnstile 手動レンダリング用 ---
	let turnstileContainer: HTMLDivElement;
	let turnstileWidgetId: string | null = null;

	// --- 専ブラ連携トークン関連の変数 ---
	let linkingToken: string | null = null;
	let linkingTokenSuccessMessage: string | null = null;
	let linkingTokenErrorMessage: string | null = null;
	let isGeneratingToken = false;
	$: formattedLinkingToken = linkingToken ? `!token(${linkingToken})` : '';

	// グローバルスコープに関数を登録する必要があるため、型定義を拡張
	declare global {
		interface Window {
			turnstile: any;
		}
	}

	// ページがブラウザにマウントされた後でのみ、クライアントサイド専用のコンポーネントを動的にインポートします。
	// これにより、サーバーサイドレンダリング(SSR)時にこのコンポーネントが読み込まれるのを防ぎ、エラーを回避します。
	onMount(async () => {
		console.log(`[診断 1/5] onMount開始時点。PUBLIC_TURNSTILE_SITE_KEY:`, PUBLIC_TURNSTILE_SITE_KEY);
		// hCaptchaコンポーネントを動的にインポート
		try {
			const hcaptchaModule = await import('svelte-hcaptcha');
			HCaptchaComponent = hcaptchaModule.default;
			hCaptchaReady = true;
		} catch (e: any) {
			console.error('hCaptchaコンポーネントの読み込みに失敗しました:', e);
			errorMessage = `コンポーネントの読み込みに失敗しました: ${e.message}`;
			step = 'error';
		}

		// Turnstileスクリプトがグローバルにロードされるのをポーリングで待機
		turnstileInterval = setInterval(() => {
			if (window.turnstile) {
				clearInterval(turnstileInterval);
				turnstileScriptReady = true;
			}
		}, 100);
	});

	onDestroy(() => {
		// コンポーネントが破棄される際に、インターバルとウィジェットをクリーンアップ
		if (turnstileInterval) clearInterval(turnstileInterval);
		if (turnstileWidgetId && window.turnstile) window.turnstile.remove(turnstileWidgetId);
	});

	// リアクティブ宣言: step, turnstileScriptReady, turnstileContainer のいずれかが変更されるたびに実行されます。
	// これにより、すべての条件が揃った正しいタイミングでのみレンダリングが実行されます。
	$: if (step === 'agreement_and_challenges' && turnstileScriptReady && turnstileContainer && !turnstileWidgetId) {
		console.log('[Turnstile手動診断 3/4] 条件が揃ったため、Turnstileのレンダリングを開始します。');
		turnstileWidgetId = window.turnstile.render(turnstileContainer, {
			sitekey: PUBLIC_TURNSTILE_SITE_KEY,
			callback: (token: string) => {
				console.log('[Turnstile手動診断 4/4] Turnstile successコールバックが呼び出されました。');
				turnstileToken = token;
			},
			'error-callback': () => {
				console.error('[Turnstile手動診断エラー] error-callbackが呼び出されました。');
				errorMessage = 'Turnstileチャレンジでエラーが発生しました。';
			}
		});
	}

	/**
	 * TurnstileとhCaptchaの検証成功後、バックエンドで事前検証を行います。
	 */
	async function handlePreflightCheck() {
		if (!hcaptchaToken || !turnstileToken || !termsAgreed) {
			errorMessage = '両方のチャレンジを完了してください。';
			return;
		}

		// --- アクセシビリティ警告の対処 ---
		// hCaptcha要素が非表示になる前に、フォーカスをコンテナ要素に移動させます。
		// これにより、フォーカスが当たっている要素が隠されるのを防ぎます。
		if (registrationContainer) {
			registrationContainer.focus();
		}

		step = 'loading';
		loading = true;
		errorMessage = '';
		infoMessage = 'ボットでないことを確認しています...';

		try {
			const fingerprintData = await getRawFingerprintData();

			const response = await fetch('/api/auth/preflight', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					hcaptcha_token: hcaptchaToken,
					turnstile_token: turnstileToken, // Turnstileのトークンも送信
					fingerprintData: fingerprintData // 仕様に基づき、この段階で情報を送信
				})
			});

			const data = await response.json();

			if (!response.ok) {
				// バックエンドからのエラーメッセージを優先して表示
				throw new Error(data.error || '事前検証に失敗しました。ページをリロードしてください。');
			}

			preflightToken = data.preflight_token;
			infoMessage = data.message || '確認が完了しました。アカウントIDを入力または新規発行してください。';
			step = 'account_id_input';
		} catch (err: any) {
			errorMessage = err.message;
			step = 'error';
		} finally {
			loading = false;
		}
	}

	// メール認証は使用しないため、関数全体をコメントアウト
	// async function handleRequestOtp() {
	// 	if (!email) {
	// 		errorMessage = 'メールアドレスを入力してください。';
	// 		return;
	// 	}
	// 	loading = true;
	// 	errorMessage = '';
	// 	infoMessage = '確認コードを送信しています...';

	// 	try {
	// 		const response = await fetch('/api/auth/request-otp', {
	// 			method: 'POST',
	// 			headers: { 'Content-Type': 'application/json' },
	// 			body: JSON.stringify({
	// 				email: email,
	// 				preflight_token: preflightToken // 保存しておいたトークンを使用
	// 			})
	// 		});

	// 		const data = await response.json();

	// 		if (!response.ok) {
	// 			throw new Error(data.error || '確認コードの送信に失敗しました。');
	// 		}

	// 		// OTP入力ページにメールアドレスを渡して遷移
	// 		await goto(`/auth/verify-otp?email=${encodeURIComponent(email)}`);
	// 	} catch (err: any) {
	// 		errorMessage = err.message;
	// 		step = 'email_input'; // エラーが発生したらメール入力に戻す
	// 	} finally {
	// 		loading = false;
	// 	}
	// }

	async function handleLoginWithAccountId() {
		if (!accountId) {
			errorMessage = 'アカウントIDを入力してください。';
			return;
		}
		loading = true;
		errorMessage = '';
		infoMessage = 'ログインしています...';

		try {
			const response = await fetch('/api/auth/login-with-account-id', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					account_id: accountId,
					preflight_token: preflightToken
				})
			});
			const data = await response.json();
			if (!response.ok) {
				throw new Error(data.error || 'ログインに失敗しました。');
			}
			// ログイン成功後、トップページへリダイレクトします。ページ全体をリロードして
			// 確実に認証状態を反映させるために window.location.href を使用します。
			window.location.href = '/';
		} catch (err: any) {
			errorMessage = err.message;
			// エラー時はアカウントID入力画面に戻す
			step = 'account_id_input';
		} finally {
			loading = false;
		}
	}

	async function handleCreateAccount() {
		loading = true;
		errorMessage = '';
		infoMessage = '新しいアカウントを発行しています...';
		try {
			const response = await fetch('/api/auth/create-account', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ preflight_token: preflightToken })
			});
			const data = await response.json();
			if (!response.ok) {
				throw new Error(data.error || 'アカウントの作成に失敗しました。');
			}
			accountId = data.account_id;
			step = 'account_id_display';
		} catch (err: any) {
			errorMessage = err.message;
			step = 'error';
		} finally {
			loading = false;
		}
	}

	async function copyAccountId() {
		if (!accountId) return;
		try {
			await navigator.clipboard.writeText(accountId);
			alert('アカウントIDをクリップボードにコピーしました。');
		} catch (err) {
			console.error('クリップボードへのコピーに失敗しました:', err);
			alert('コピーに失敗しました。手動でコピーしてください。');
		}
	}

	/**
	 * 専ブラ連携トークンを発行する
	 */
	async function handleGenerateLinkingToken() {
		isGeneratingToken = true;
		linkingTokenSuccessMessage = null;
		linkingTokenErrorMessage = null;

		try {
			// この時点でユーザーはログイン済みのはずなので、Cookieは自動で送信される
			const response = await fetch('/api/auth/me/regenerate-linking-token', {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json'
				}
			});

			const data = await response.json();

			if (!response.ok) {
				linkingTokenErrorMessage = data.error || `エラーが発生しました (HTTP ${response.status})`;
			} else {
				linkingTokenSuccessMessage = data.message;
				linkingToken = data.linking_token;
			}
		} catch (error) {
			linkingTokenErrorMessage = '通信エラーが発生しました。';
			console.error('Failed to regenerate linking token:', error);
		} finally {
			isGeneratingToken = false;
		}
	}

	/**
	 * 専ブラ連携トークンをクリップボードにコピーする
	 */
	function copyLinkingToken() {
		if (!formattedLinkingToken) return;
		navigator.clipboard.writeText(formattedLinkingToken).then(
			() => {
				alert('専ブラ連携トークンをクリップボードにコピーしました。');
			},
			(err) => {
				alert('クリップボードへのコピーに失敗しました。');
				console.error('Could not copy text: ', err);
			}
		);
	}

	async function handleReturnToTop() {
		// goto('/') はクライアントサイドナビゲーションのため、ページ全体をリロードして
		// 確実にログイン状態を反映させるために window.location.href を使用します。
		window.location.href = '/';
	}
</script>

<div class="registration-container" bind:this={registrationContainer} tabindex="-1">

	<h1>アカウント認証</h1>
	{#if loading}
		<p>{infoMessage}</p>
		<!-- ここにスピナーなどのUIを配置 -->
	{/if}
	
	<!-- 1. 同意とチャレンジのステップ -->
	{#if step === 'agreement_and_challenges'}
		<p>掲示板へ書き込むためには、以下の認証を完了し、利用規約及びプライバシーポリシーに同意してください。
			<br>VPN、プロキシは切断してください。</p>	
		<div class="challenges-container">
			<div class="challenge-item">
				<!-- Turnstileを描画するためのコンテナ -->
				<div bind:this={turnstileContainer} />
			</div>
			<div class="challenge-item">
				{#if hCaptchaReady}
					<svelte:component
						this={HCaptchaComponent}
						sitekey={PUBLIC_HCAPTCHA_SITE_KEY}
						on:success={(e) => {
							hcaptchaToken = e.detail.token;
						}}
						on:error={(e) => {
							errorMessage = `hCaptchaエラー: ${e.detail.error}`;
						}}
					/>
				{:else}
					<p>hCaptchaを読み込んでいます...</p>
				{/if}
			</div>
			<div class="challenge-item">
				<label class="agree-label">
					<input type="checkbox" bind:checked={termsAgreed} />
					<span>
						<a href="/terms" target="_blank" rel="noopener noreferrer">利用規約及びプライバシーポリシー</a>に同意します。
					</span>
				</label>
			</div>
		</div>
		<button
			on:click={handlePreflightCheck}
			disabled={!turnstileToken || !hcaptchaToken || !termsAgreed}
		>
			認証へ進む
		</button>
	{/if}

	<!-- 2. アカウントID入力/作成ステップ -->
	{#if step === 'account_id_input'}
		<p>{infoMessage}</p>
		<div class="account-id-form">
			<form on:submit|preventDefault={handleLoginWithAccountId} class="form-group">
				<label for="account-id-input">既にお持ちのアカウントID:</label>
				<input id="account-id-input" type="text" placeholder="アカウントIDを入力" bind:value={accountId} required />
				<button type="submit" disabled={loading}>ログイン</button>
			</form>
			<div class="divider">または</div>
			<div class="form-group">
				<p>初めての方はこちらから新しいアカウントIDを発行してください。</p>
				<button on:click={handleCreateAccount} disabled={loading}>新規アカウントIDを発行</button>
			</div>
		</div>
	{/if}

	<!-- 3. 新規アカウントID表示ステップ -->
	{#if step === 'account_id_display'}
		<div class="success-container">
			<h2>アカウントIDが発行されました</h2>
			<p>このアカウントIDはあなた専用のものです。**絶対に他人に教えたり、なくしたりしないでください。**</p>
			<p>次回以降、このIDを使ってログインできます。安全な場所に保管してください。</p>
			
			<div class="token-display">
				<input type="text" readonly value={accountId} />
				<button on:click={copyAccountId}>コピー</button>
			</div>

			<button on:click={() => step = 'linking_token'}>
				次に専ブラ連携トークンを発行する
			</button>
		</div>
	{/if}

	<!-- 4. 専ブラ連携トークン発行ステップ -->
	{#if step === 'linking_token'}
		<div class="linking-token-container">
			<h2>専ブラ連携トークン発行</h2>
			<p>
				お使いの専ブラで当掲示板の適当なスレッドを開き、以下のトークンを貼り付けてそのままレス投稿してください。
			</p>
			<p>
				このページを飛ばしても、専ブラ連携トークンは後から何度でも発行できます。
			</p>

			<button on:click={handleGenerateLinkingToken} disabled={isGeneratingToken}>
				{isGeneratingToken ? '発行中...' : '連携トークンを発行する'}
			</button>

			{#if linkingTokenSuccessMessage}
				<p class="success" transition:fly={{ y: -10, duration: 300, easing: quintOut }}>{linkingTokenSuccessMessage}</p>
			{/if}
			{#if linkingTokenErrorMessage}
				<p class="error" transition:fly={{ y: -10, duration: 300, easing: quintOut }}>{linkingTokenErrorMessage}</p>
			{/if}

			{#if linkingToken}
				<div class="token-display" transition:fly={{ y: 10, duration: 300, easing: quintOut }}>
					<input type="text" readonly value={formattedLinkingToken} />
					<button on:click={copyLinkingToken}>コピー</button>
				</div>
			{/if}

			<button class="secondary" on:click={handleReturnToTop}>トップページへ戻る</button>
		</div>
	{/if}

	<!-- 共通のエラー表示エリア -->
	{#if errorMessage}
		<p class="error">{errorMessage}</p>
	{/if}

	<!-- エラー発生時の再試行ステップ -->
	{#if step === 'error'}
		<button on:click={() => {
				// ページ全体をリロードして、hCaptchaやTurnstileの状態を含めて
				// 完全に初期化します。
				window.location.reload();
		}}>再試行</button>
	{/if}
</div>

<style>
	.registration-container {
		width: 100%; /* コンテナが利用可能な幅いっぱいに広がるように設定 */
		box-sizing: border-box; /* paddingとborderをwidthに含める */
		max-width: 600px; /* 横幅を広げて、利用規約を読みやすくします */
		margin: 2rem auto;
		padding: 2rem;
		border: 1px solid #ccc;
		border-radius: 8px;
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}
	.agree-label {
		display: flex;
		align-items: center;
		justify-content: center; /* ラベル内の要素（チェックボックスとテキスト）を中央揃えにします */
		gap: 0.8rem; /* チェックボックスとテキストの間隔を少し広げます */
		font-size: 1.1rem; /* テキストを少し大きくします */
		margin: 1rem 0; /* 上下の余白を調整して、ボタンとの間隔を確保します */
	}
	.info-text {
		font-size: 0.8rem;
		color: #666;
	}
	.challenges-container {
		display: flex;
		flex-direction: column;
		gap: 1.5rem;
	}
	.challenge-item {
		display: flex;
		flex-direction: column;
		align-items: center; /* 中央揃え */
		gap: 0.5rem;
		border: 1px solid #eee;
		padding: 1rem;
		border-radius: 8px;
		width: 100%;
	}
	.completed-check {
		color: green;
		margin-left: 0.5rem;
	}
	.error {
		color: red;
	}
	.success {
		color: green;
		font-weight: bold;
	}
	input[type='email'], input[type='text'] {
		width: 100%;
		padding: 0.5rem;
		margin: 0.5rem 0 1rem 0;
		box-sizing: border-box; /* paddingを含めて幅を100%にする */
	}
	input[type='checkbox'] {
		/* チェックボックスのスタイルを調整 */
		width: auto; /* 幅を自動に設定 */
		margin: 0; /* 不要なマージンを削除 */
		transform: scale(1.5); /* チェックボックス自体を1.5倍に拡大します */
	}
	.agree-label a {
		color: #007bff;
		text-decoration: none;
	}
	/* プログラムによってフォーカスされた際の青い枠線を非表示にする */
	.registration-container:focus {
		outline: none;
	}

	button {
		padding: 0.75rem;
		border: none;
		border-radius: 4px;
		background-color: #007bff;
		color: white;
		cursor: pointer;
		font-size: 1rem;
		transition: background-color 0.2s;
		width: 100%; /* ボタンの幅を統一 */
		box-sizing: border-box;
	}
	button:disabled {
		background-color: #ccc;
		cursor: not-allowed;
	}
	button:not(:disabled):hover {
		background-color: #0056b3;
	}

	.account-id-form {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}
	.form-group {
		border: 1px solid #eee;
		padding: 1.5rem;
		border-radius: 8px;
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}
	.divider {
		text-align: center;
		color: #888;
	}
	.success-container {
		text-align: center;
	}
	.linking-token-container {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}
	.token-display {
		display: flex;
		justify-content: center;
		margin: 1rem 0;
		gap: 0.5rem;
	}
	.token-display input {
		flex-grow: 1;
		font-family: monospace;
	}
	.token-display button {
		width: auto; /* コピーボタンの幅は自動 */
		flex-shrink: 0;
	}
	button.secondary {
		background-color: #6c757d;
	}
	button.secondary:hover {
		background-color: #5a6268;
	}

	/* スマートフォンなどの小さい画面向けの調整 */
	@media (max-width: 768px) {
		.registration-container {
			padding: 1rem; /* 画面が狭い場合は、左右の余白を減らす */
			margin: 1rem auto;
		}
		.form-group {
			padding: 1rem;
		}
	}
</style>