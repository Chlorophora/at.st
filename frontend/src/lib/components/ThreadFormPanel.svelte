<script lang="ts">
	import { fly } from 'svelte/transition';
	import { goto } from '$app/navigation';
	import { getVisitorId } from '$lib/utils/fingerprint';
	import { isThreadFormPanelOpen, closeThreadFormPanel } from '$lib/stores/ui';

	// このパネルが表示される板のIDをプロパティとして受け取る
	export let boardId: number;

	let newTitle = '';
	let newBody = '';
	let newAuthorName = '';
	let submitting = false;
	let errors: { title?: string; body?: string; author_name?: string; general?: string } = {};

	// パネルが開かれたときにタイトルにフォーカスを当てる
	let titleElement: HTMLInputElement;
	$: if ($isThreadFormPanelOpen && titleElement) {
		setTimeout(() => titleElement.focus(), 50);
	}

	async function handleSubmitThread() {
		if (!boardId || submitting) return;

		submitting = true;
		errors = {}; // エラーをリセット
		try {
			const fingerprint = await getVisitorId();

			const payload: any = {
				title: newTitle,
				body: newBody,
				board_id: boardId,
				fingerprint
			};
			if (newAuthorName.trim()) {
				payload.author_name = newAuthorName.trim();
			}

			const response = await fetch('/api/posts', {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json'
				},
				body: JSON.stringify(payload)
			});

			if (!response.ok) {
				// レスポンスボディがあるか確認してからjson()を呼ぶ
				const contentType = response.headers.get('content-type');
				if (contentType && contentType.includes('application/json')) {
					const errorData = await response.json();
					if (errorData.details) {
						for (const [field, fieldErrors] of Object.entries(errorData.details)) {
							if (field === 'title' || field === 'body' || field === 'author_name') {
								errors[field] = (fieldErrors as any[])[0]?.message;
							}
						}
					} else {
						errors.general = errorData.error || 'スレッド作成エラー';
					}
				} else {
					// JSONでない、またはボディが空の場合
					errors.general = `サーバーエラーが発生しました (ステータス: ${response.status})。`;
				}
				return;
			}

			// 成功した場合
			const newPost = await response.json();
			newTitle = '';
			newBody = '';
			newAuthorName = '';
			errors = {};
			closeThreadFormPanel(); // パネルを閉じる
			await goto(`/posts/${newPost.id}`); // 作成されたスレッドに遷移
		} catch (error: any) {
			console.error('スレッド作成エラー:', error);
			errors.general = error.message;
		} finally {
			submitting = false;
		}
	}
</script>

{#if $isThreadFormPanelOpen}
	<div class="thread-form-panel" transition:fly={{ duration: 300, y: 100 }}>
		<div class="panel-header">
			<h3>新しいスレッドを投稿</h3>
			<button class="close-button" on:click={closeThreadFormPanel} title="閉じる">
				<!-- × アイコン -->
				<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
			</button>
		</div>
		<div class="panel-body">
			<form on:submit|preventDefault={handleSubmitThread}>
				<div>
					<label for="panelThreadTitle">タイトル:</label>
					<div class="input-wrapper">
						<input
							type="text"
							id="panelThreadTitle"
							bind:this={titleElement}
							bind:value={newTitle}
							required
							maxlength="100"
							disabled={submitting}
						/>
						<span class="char-counter">{newTitle.length} / 100</span>
					</div>
					{#if errors.title}
						<p class="error-message field-error">{errors.title}</p>
					{/if}
				</div>
				<div>
					<label for="panelThreadAuthor">名前:</label>
					<div class="input-wrapper">
						<input
							type="text"
							id="panelThreadAuthor"
							bind:value={newAuthorName}
							placeholder="野球民"
							maxlength="10"
							disabled={submitting}
						/>
						<span class="char-counter">{newAuthorName.length} / 10</span>
					</div>
					{#if errors.author_name}
						<p class="error-message field-error">{errors.author_name}</p>
					{/if}
				</div>
				<div>
					<label for="panelThreadBody">本文:</label>
					<div class="input-wrapper">
						<textarea
							id="panelThreadBody"
							bind:value={newBody}
							required
							rows="8"
							maxlength="750"
							disabled={submitting}
						></textarea>
						<span class="char-counter">{newBody.length} / 750</span>
					</div>
					{#if errors.body}
						<p class="error-message field-error">{errors.body}</p>
					{/if}
				</div>
				<button type="submit" disabled={!newTitle || !newBody || submitting}>
					{submitting ? '投稿中...' : '投稿する'}
				</button>
				{#if errors.general}
					<p class="error-message">{errors.general}</p>
				{/if}
			</form>
		</div>
	</div>
{/if}

<style>
	/* CommentFormPanel.svelte からスタイルをコピーしてクラス名を変更 */
	.thread-form-panel {
		position: fixed;
		bottom: 1rem;
		right: 1rem;
		width: 90vw;
		max-width: 450px;
		background-color: white;
		border-radius: 12px;
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.2);
		z-index: 1010; /* ヘッダー(1000)より手前に表示 */
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.panel-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.75rem 1.5rem;
		border-bottom: 1px solid #e0e0e0;
		background-color: #f7f7f7;
	}

	.panel-header h3 {
		margin: 0;
		font-size: 1.1rem;
	}

	.close-button {
		background: none;
		border: none;
		cursor: pointer;
		padding: 0.5rem;
		margin: -0.5rem; /* クリック領域を広げる */
		color: #666;
		line-height: 0;
	}
	.close-button:hover {
		color: #000;
	}

	.panel-body {
		padding: 1.5rem;
		overflow-y: auto;
	}

	/* フォームのスタイルは [id]/+page.svelte からコピー＆調整 */
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
		width: 100%;
		box-sizing: border-box;
		background-color: #fff;
	}
	form textarea {
		resize: vertical;
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
		white-space: pre-wrap;
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
		background-color: rgba(255, 255, 255, 0.8);
		padding: 1px 4px;
		border-radius: 3px;
		pointer-events: none;
	}
	form textarea + .char-counter {
		bottom: 12px;
	}

	@media (max-width: 768px) {
		.thread-form-panel {
			bottom: 0;
			right: 0;
			width: 100%;
			border-radius: 12px 12px 0 0;
		}
	}
</style>