<script lang="ts">
	import { fly } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';

	let isLoading = false;
	let newToken: string | null = null;
	let successMessage: string | null = null;
	let errorMessage: string | null = null;

	// APIから受け取った生のトークンを整形するためのリアクティブな変数
	$: formattedToken = newToken ? `!token(${newToken})` : '';

	async function regenerateToken() {
		isLoading = true;
		successMessage = null;
		errorMessage = null;

		try {
			const response = await fetch('/api/auth/me/regenerate-linking-token', {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json'
				}
			});

			const data = await response.json();

			if (!response.ok) {
				// APIからのエラーメッセージを優先して表示
				errorMessage = data.error || `エラーが発生しました (HTTP ${response.status})`;
			} else {
				// 成功した場合にのみ、新しいトークンで上書きします
				successMessage = data.message;
				newToken = data.linking_token;
			}
		} catch (error) {
			errorMessage = '通信エラーが発生しました。';
			console.error('Failed to regenerate token:', error);
		} finally {
			isLoading = false;
		}
	}

	function copyToClipboard() {
		if (!formattedToken) return;
		navigator.clipboard.writeText(formattedToken).then(
			() => {
				alert('トークンをクリップボードにコピーしました。');
			},
			(err) => {
				alert('クリップボードへのコピーに失敗しました。');
				console.error('Could not copy text: ', err);
			}
		);
	}
</script>

<div class="token-regenerator">
	<h3>専ブラ連携トークン発行</h3>
	<p>
		専ブラにて、当掲示板の適当なスレッドにこのトークンを貼り付けてそのままレス投稿してください。
	</p>
	<p class="note">
		
	</p>

	<button on:click={regenerateToken} disabled={isLoading}>
		{isLoading ? '発行中...' : '新しいトークンを発行する'}
	</button>

	{#if successMessage}
		<div class="message success" transition:fly={{ y: -10, duration: 300, easing: quintOut }}>
			{successMessage}
		</div>
	{/if}

	{#if errorMessage}
		<div class="message error" transition:fly={{ y: -10, duration: 300, easing: quintOut }}>
			{errorMessage}
		</div>
	{/if}

	{#if newToken}
		<div class="token-display" transition:fly={{ y: 10, duration: 300, easing: quintOut }}>
			<input type="text" readonly value={formattedToken} />
			<button on:click={copyToClipboard} title="コピー">コピー</button>
		</div>
	{/if}
</div>

<style>
	.token-regenerator {
		border: 1px solid #ccc;
		padding: 1.5rem;
		border-radius: 8px;
		background-color: #f9f9f9;
		max-width: 600px;
		margin: 1rem 0;
	}
	h3 { margin-top: 0; color: #333; }
	p { color: #555; line-height: 1.6; margin-bottom: 1.5rem; }
	.note { font-size: 0.9rem; color: #777; border-left: 3px solid #ddd; padding-left: 1rem; }
	button { background-color: #007bff; color: white; border: none; padding: 10px 15px; border-radius: 5px; cursor: pointer; font-size: 1rem; transition: background-color 0.2s; }
	button:disabled { background-color: #aaa; cursor: not-allowed; }
	button:not(:disabled):hover { background-color: #0056b3; }
	.message { padding: 1rem; margin-top: 1rem; border-radius: 5px; border: 1px solid; }
	.success { background-color: #d4edda; color: #155724; border-color: #c3e6cb; }
	.error { background-color: #f8d7da; color: #721c24; border-color: #f5c6cb; }
	.token-display { display: flex; align-items: center; margin-top: 1rem; }
	.token-display input { flex-grow: 1; padding: 8px; font-family: monospace; border: 1px solid #ccc; border-radius: 4px; background-color: #fff; }
	.token-display button { margin-left: 0.5rem; padding: 8px 12px; background-color: #6c757d; }
    .token-display button:hover { background-color: #5a6268; }
</style>