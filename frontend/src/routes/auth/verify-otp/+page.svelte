<script lang="ts">
	import { goto, invalidateAll } from '$app/navigation';
	import type { PageData } from './$types';

	export let data: PageData;
	const { email } = data;

	// UIの状態を管理するための変数 ('form' または 'success')
	let step: 'form' | 'success' = 'form';

	let otp = '';
	let isLoading = false;
	let errorMessage = '';
	// 認証成功後に受け取る連携トークンを保存する変数
	let linkingToken = '';

	// 表示・コピー用に `!token(...)` 形式にフォーマットしたトークン
	$: formattedToken = linkingToken ? `!token(${linkingToken})` : '';

	async function handleVerifyOtp() {
		if (!otp || otp.length !== 6) {
			errorMessage = '6桁のコードを入力してください。';
			return;
		}
		isLoading = true;
		errorMessage = '';

		try {
			// hooks.server.ts のプロキシを活用するため、相対パスを使用します。
			// これにより、APIのURLを一元管理でき、CORSの問題も回避できます。
			const response = await fetch('/api/auth/verify-otp', {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json'
				},
				body: JSON.stringify({ email, otp_code: otp })
			});

			const responseData = await response.json();

			if (response.ok) {
				// レスポンスから連携トークンを取得
				linkingToken = responseData.linking_token;
				// 表示を「認証成功」画面に切り替える
				step = 'success';

				// 自動的なデータ再読み込み（invalidateAll）を削除します。
				// これにより、このページに留まったままトークンを表示できます。
				// ユーザーが「トップページへ戻る」リンクをクリックした際に、SvelteKitが新しい認証状態でページを読み込むため、ヘッダーなどのUIは自然に更新されます。
			} else {
				errorMessage = responseData.error || 'OTPの検証に失敗しました。';
			}
		} catch (error) {
			console.error('Error verifying OTP:', error);
			errorMessage = 'サーバーとの通信中にエラーが発生しました。';
		} finally {
			isLoading = false;
		}
	}

	// トークンをクリップボードにコピーする関数
	async function copyToken() {
		if (!formattedToken) return;
		try {
			await navigator.clipboard.writeText(formattedToken);
			alert('トークンをクリップボードにコピーしました。');
		} catch (err) {
			console.error('クリップボードへのコピーに失敗しました:', err);
			alert('コピーに失敗しました。手動でコピーしてください。');
		}
	}

	// トップページに戻る際に、全データを再検証してUIを更新する
	async function handleReturnToTop() {
		await goto('/'); // トップページへ遷移します。SvelteKitが自動でデータを再取得します。
	}
</script>

<div class="container">
	{#if step === 'form'}
		<h1>OTP認証</h1>
		<p>{email} に送信された6桁のコードを入力してください。</p>

		<form on:submit|preventDefault={handleVerifyOtp}>
			<div class="form-group">
				<label for="otp">認証コード</label>
				<input
					type="tel"
					id="otp"
					inputmode="numeric"
					bind:value={otp}
					maxlength="6"
					required
					disabled={isLoading}
					placeholder="123456"
				/>
			</div>

			{#if errorMessage}
				<p class="error">{errorMessage}</p>
			{/if}

			<button type="submit" disabled={isLoading}>
				{isLoading ? '検証中...' : '認証'}
			</button>
		</form>
	{:else if step === 'success'}
		<div class="success-container">
			<h2>専ブラ連帯用トークン</h2>
			<p>専ブラにて以下のトークンを当掲示板の任意のスレッドに貼り付けて、そのままレス投稿してください。</p>
			<p class="important-notice">
				<strong>有効期限１０分</strong>
			</p>

			<div class="token-display">
				<input type="text" readonly value={formattedToken} />
				<button on:click={copyToken}>コピー</button>
			</div>

			<a href="/" class="button-link" on:click|preventDefault={handleReturnToTop}>トップページへ戻る</a>
		</div>
	{/if}
</div>

<style>
	.container {
		max-width: 500px;
		margin: 2rem auto;
		padding: 2rem;
		border: 1px solid #ccc;
		border-radius: 8px;
	}
	.success-container {
		text-align: center;
	}
	.important-notice {
		color: #d32f2f;
		background-color: #ffebee;
		padding: 0.5rem;
		border-radius: 4px;
		margin: 1rem 0;
	}
	.token-display {
		display: flex;
		justify-content: center;
		margin: 1rem 0;
		gap: 0.5rem;
	}
	.token-display input {
		width: 300px;
		font-family: monospace;
	}
	.button-link {
		display: inline-block;
		margin-top: 1rem;
		padding: 0.75rem 1.5rem;
		border-radius: 4px;
		background-color: #2d8cff;
		color: white;
		text-decoration: none;
		transition: background-color 0.2s;
	}
	.button-link:hover {
		background-color: #0070e0;
	}
	.error {
		color: red;
	}
</style>