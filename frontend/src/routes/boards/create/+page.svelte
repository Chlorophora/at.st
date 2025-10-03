<script lang="ts">
	import { invalidateAll, goto } from '$app/navigation';
	import { getVisitorId } from '$lib/utils/fingerprint';

	let newBoardName = '';
	let newBoardDescription = '';
	let newBoardDefaultName = '';
	let submitting = false;
	let errors: { name?: string; description?: string; default_name?: string; general?: string } = {};

	async function handleCreateBoard() {
		if (submitting) return;
		submitting = true;
		errors = {}; // エラーをリセット

		try {
			// フォーム送信時にフィンガープリントを取得
			const fingerprint = await getVisitorId();

			const body: { name: string; description: string; default_name?: string; fingerprint: string } = {
				name: newBoardName,
				description: newBoardDescription,
				fingerprint
			};

			if (newBoardDefaultName.trim()) {
				body.default_name = newBoardDefaultName.trim();
			}

			const response = await fetch('/api/boards', {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json'
				},
				// credentials: 'include' は不要になります。相対パスへのfetchは同一オリジンリクエストとなり、Cookieはブラウザが自動で送信します。
				body: JSON.stringify(body)
			});

			if (!response.ok) {
				if (response.status === 401) {
					await goto('/auth');
					return;
				}
				const errorData = await response.json();
				if (errorData.details) {
					// バリデーションエラーをフィールドごとに格納
					for (const [field, fieldErrors] of Object.entries(errorData.details)) {
						if (field === 'name' || field === 'description' || field === 'default_name') {
							errors[field] = (fieldErrors as any[])[0]?.message;
						}
					}
				} else {
					// バリデーション以外のエラー
					errors.general = errorData.error || '板の作成に失敗しました。';
				}
				return; // エラーがあった場合はここで処理を終了
			}

			// 成功時の処理
			const newBoard = await response.json();
			// 作成した板のページにリダイレクト
			await goto(`/boards/${newBoard.id}`);
		} catch (error: any) {
			console.error('板作成エラー:', error);
			errors.general = error.message;
		} finally {
			submitting = false;
		}
	}
</script>

<div class="create-board-container">
	<h1>新しい板を作成</h1>
	<p class="page-description">
		ここでは新しい板を作成できます。板の名前、板の説明、デフォルト名は全て後から変更できます。使用実態の無い板はスレッドと同様にアーカイブ化（新規スレ建てを停止）されることを留意してください。
	</p>
	<form on:submit|preventDefault={handleCreateBoard}>
		<div>
			<label for="boardName">板の名前:</label>
			<div class="input-wrapper">
				<input type="text" id="boardName" bind:value={newBoardName} required maxlength="20" disabled={submitting} />
				<span class="char-counter">{newBoardName.length} / 20</span>
			</div>
			{#if errors.name}
				<p class="error-message field-error">{errors.name}</p>
			{/if}
		</div>
		<div>
			<label for="boardDescription">板の説明:</label>
			<div class="input-wrapper">
				<textarea
					id="boardDescription"
					bind:value={newBoardDescription}
					placeholder="ローカルルールなど"
					required
					rows="3"
					maxlength="100"
					disabled={submitting}
				></textarea>
				<span class="char-counter">{newBoardDescription.length} / 100</span>
			</div>
			{#if errors.description}
				<p class="error-message field-error">{errors.description}</p>
			{/if}
		</div>
		<div>
			<label for="boardDefaultName">デフォルト名 (任意):</label>
			<div class="input-wrapper">
				<input
					type="text"
					id="boardDefaultName"
					bind:value={newBoardDefaultName}
					placeholder="（例: 野球民）"
					maxlength="10"
					disabled={submitting}
				/>
				<span class="char-counter">{newBoardDefaultName.length} / 10</span>
			</div>
			{#if errors.default_name}
				<p class="error-message field-error">{errors.default_name}</p>
			{/if}
		</div>
		<button type="submit" disabled={!newBoardName || !newBoardDescription || submitting}>
			{submitting ? '作成中...' : '作成する'}
		</button>
		{#if errors.general}
			<p class="error-message">{errors.general}</p>
		{/if}
	</form>
</div>

<style>
	.create-board-container {
		max-width: 600px;
		margin: 2rem auto;
		padding: 1.5rem;
		border: 1px solid #ddd;
		border-radius: 8px;
		background-color: #f9f9f9;
	}
	h1 {
		text-align: center;
		margin-bottom: 1.5rem;
	}
	.page-description {
		text-align: left;
		margin-bottom: 1.5rem;
		color: #555;
	}
	form {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}
	form div {
		display: flex;
		flex-direction: column;
	}
	form label {
		margin-bottom: 0.5rem;
		font-weight: 500;
	}
	form input,
	form textarea {
		padding: 0.75rem;
		border: 1px solid #ccc;
		border-radius: 4px;
		font-size: 1rem;
	}
	form button {
		padding: 0.75rem 1.5rem;
		border-radius: 4px;
		border: none;
		background-color: #2d8cff;
		color: white;
		font-size: 1rem;
		cursor: pointer;
		transition: background-color 0.2s;
	}
	form button:hover {
		background-color: #0070e0;
	}
	form button:disabled {
		background-color: #ccc;
		cursor: not-allowed;
	}
	.error-message {
		color: red;
		margin-top: 0.5rem;
	}
	.field-error {
		font-size: 0.9em;
		margin-top: 0.25rem;
	}

	.input-wrapper {
		position: relative;
	}

	.char-counter {
		position: absolute;
		bottom: 8px;
		right: 8px;
		font-size: 0.8em;
		color: #666;
		background-color: rgba(249, 249, 249, 0.8); /* フォーム背景色に合わせる */
		padding: 1px 4px;
		border-radius: 3px;
		pointer-events: none; /* カウンターがテキストエリアの操作を妨げないように */
	}
	form textarea + .char-counter {
		bottom: 12px; /* テキストエリアはリサイズハンドルがあるので少し上に */
	}
</style>
