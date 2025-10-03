<script lang="ts">
	import { createEventDispatcher } from 'svelte';
	import { page } from '$app/stores';
	import type { Board } from 'src/app';

	export let board: Board;

	const dispatch = createEventDispatcher();

	$: isAdmin = $page.data.user?.role === 'admin';

	// --- Basic Details State ---
	let name: string;
	let description: string;
	let defaultName: string;
	let isDetailsLoading = false;
	let detailsApiMessage: { type: 'success' | 'error'; text: string } | null = null;

	// --- Max Posts State ---
	let maxPosts = board.max_posts;
	let isMaxPostsLoading = false;
	let maxPostsMessage: { type: 'success' | 'error'; text: string } | null = null;

	// --- Moderation Type State ---
	let moderationType: 'alpha' | 'beta';
	let isModTypeLoading = false;
	let modTypeMessage: { type: 'success' | 'error'; text: string } | null = null;

	// --- Auto Archive State ---
	let autoArchiveEnabled: boolean;
	let isAutoArchiveLoading = false;
	let autoArchiveMessage: { type: 'success' | 'error'; text: string } | null = null;

	// --- Logic for Basic Details ---
	$: isDirty =
		name !== board.name || description !== board.description || defaultName !== board.default_name;
	$: canSubmitDetails = isDirty && !isDetailsLoading;

	async function handleSubmitDetails() {
		if (!canSubmitDetails) return;

		isDetailsLoading = true;
		detailsApiMessage = null;

		const payload: { name?: string; description?: string; default_name?: string } = {};
		if (name !== board.name) payload.name = name;
		if (description !== board.description) payload.description = description;
		if (defaultName !== board.default_name) payload.default_name = defaultName;

		try {
			const response = await fetch(`/api/boards/${board.id}/details`, {
				method: 'PATCH',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify(payload)
			});

			const responseData = await response.json();

			if (!response.ok) {
				throw new Error(responseData.error || '更新に失敗しました。');
			}

			detailsApiMessage = { type: 'success', text: '板の基本設定を更新しました。' };
			dispatch('update', responseData);
		} catch (error: any) {
			detailsApiMessage = { type: 'error', text: error.message };
		} finally {
			isDetailsLoading = false;
		}
	}

	// --- Logic for Max Posts ---
	async function handleSaveMaxPosts() {
		isMaxPostsLoading = true;
		maxPostsMessage = null;

		try {
			const response = await fetch(`/api/admin/boards/${board.id}/max-posts`, {
				method: 'PATCH',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ max_posts: Number(maxPosts) })
			});

			if (!response.ok) {
				const err = await response.json().catch(() => ({}));
				throw new Error(err.error || `更新に失敗しました (HTTP ${response.status})`);
			}

			const updatedBoard = await response.json();
			maxPostsMessage = { type: 'success', text: 'スレッド数上限を更新しました。' };
			dispatch('update', updatedBoard);
		} catch (error: any) {
			maxPostsMessage = { type: 'error', text: error.message };
		} finally {
			isMaxPostsLoading = false;
		}
	}

	// --- Logic for Moderation Type ---
	async function handleModTypeChange(event: Event) {
		const newType = (event.target as HTMLInputElement).value;
		if (newType === board.moderation_type) return;

		isModTypeLoading = true;
		modTypeMessage = null;
		// UIの即時反映（楽観的更新）
		moderationType = newType as 'alpha' | 'beta';

		try {
			const response = await fetch(`/api/admin/boards/${board.id}/moderation-type`, {
				method: 'PATCH',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ moderation_type: newType })
			});

			if (!response.ok) {
				const err = await response.json().catch(() => ({}));
				throw new Error(err.error || '更新に失敗しました');
			}

			const updatedBoard = await response.json();
			modTypeMessage = { type: 'success', text: 'モデレーションタイプを更新しました。' };
			dispatch('update', updatedBoard);
		} catch (e: any) {
			modTypeMessage = { type: 'error', text: e.message };
			// エラー時に元の値に戻す
			moderationType = board.moderation_type || 'alpha';
		} finally {
			isModTypeLoading = false;
		}
	}

	// --- Logic for Auto Archive ---
	async function handleToggleAutoArchive() {
		isAutoArchiveLoading = true;
		autoArchiveMessage = null;
		// Optimistic update
		autoArchiveEnabled = !autoArchiveEnabled;

		try {
			const response = await fetch(`/api/admin/boards/${board.id}/toggle-auto-archive`, {
				method: 'POST'
			});

			if (!response.ok) {
				const err = await response.json().catch(() => ({}));
				throw new Error(err.error || '更新に失敗しました');
			}

			const updatedBoard = await response.json();
			autoArchiveMessage = {
				type: 'success',
				text: `自動アーカイブを${updatedBoard.auto_archive_enabled ? '有効' : '無効'}にしました。`
			};
			dispatch('update', updatedBoard);
		} catch (e: any) {
			autoArchiveMessage = { type: 'error', text: e.message };
			// Revert on error
			autoArchiveEnabled = !autoArchiveEnabled;
		} finally {
			isAutoArchiveLoading = false;
		}
	}

	// 親から渡される board プロパティの変更を監視し、ローカルのstateを更新
	$: {
		if (board) {
			// Basic details
			name = board.name;
			description = board.description;
			defaultName = board.default_name;

			// Other settings
			maxPosts = board.max_posts;
			moderationType = board.moderation_type || 'alpha';
			autoArchiveEnabled = board.auto_archive_enabled;
		}
	}
</script>

<details class="settings-panel">
	<summary>管理者/作成者向け設定</summary>
	<div class="settings-content-wrapper">
		<!-- 板の基本設定 -->
		<div class="setting-item">
			<h4>板の基本設定</h4>
			<form on:submit|preventDefault={handleSubmitDetails} class:disabled={isDetailsLoading}>
				<div class="form-field">
					<label for="board-name">板の名前 (1-20文字)</label>
					<input id="board-name" type="text" bind:value={name} maxlength="20" required />
				</div>
				<div class="form-field">
					<label for="board-description">板の説明 (1-100文字)</label>
					<textarea
						id="board-description"
						bind:value={description}
						maxlength="100"
						rows="3"
						required
					/>
				</div>
				<div class="form-field">
					<label for="board-default-name">デフォルト名 (最大10文字)</label>
					<input id="board-default-name" type="text" bind:value={defaultName} maxlength="10" />
				</div>

				<div class="form-actions">
					<button type="submit" disabled={!canSubmitDetails}>
						{#if isDetailsLoading}
							更新中...
						{:else}
							基本設定を更新
						{/if}
					</button>
				</div>

				{#if detailsApiMessage}
					<p class="message {detailsApiMessage.type}">{detailsApiMessage.text}</p>
				{/if}
			</form>
		</div>

		<!-- モデレーションタイプ -->
		<div class="setting-item">
			<h4>モデレーションタイプ</h4>
			<div class="radio-group">
				<label class:disabled={isModTypeLoading}>
					<input
						type="radio"
						name="moderation-type-{board.id}"
						value="alpha"
						bind:group={moderationType}
						on:change={handleModTypeChange}
						disabled={isModTypeLoading}
					/>
					α
				</label>
				<label class:disabled={isModTypeLoading}>
					<input
						type="radio"
						name="moderation-type-{board.id}"
						value="beta"
						bind:group={moderationType}
						on:change={handleModTypeChange}
						disabled={isModTypeLoading}
					/>
					β (>>1にBAN権限が付与されるかも)
				</label>
			</div>
			{#if modTypeMessage}
				<p class="message {modTypeMessage.type}">{modTypeMessage.text}</p>
			{/if}
		</div>

		<!-- 管理者専用設定 -->
		{#if isAdmin}
			<div class="setting-item">
				<h4>スレッド数上限 (管理者のみ)</h4>
				<p class="description">
					この板に作成できるスレッドの最大数を設定します。上限に達すると新しいスレッドは作成できなくなります。
				</p>
				<form on:submit|preventDefault={handleSaveMaxPosts}>
					<div class="form-group">
						<label for="max-posts">上限数:</label>
						<input
							type="number"
							id="max-posts"
							bind:value={maxPosts}
							min="1"
							disabled={isMaxPostsLoading}
						/>
					</div>
					<button type="submit" disabled={isMaxPostsLoading}>
						{isMaxPostsLoading ? '保存' : '上限を保存'}
					</button>
				</form>
				{#if maxPostsMessage}
					<p class="message {maxPostsMessage.type}">{maxPostsMessage.text}</p>
				{/if}
			</div>

			<div class="setting-item">
				<h4>自動アーカイブ設定 (管理者のみ)</h4>
				<p class="description">
					この板が自動アーカイブ（非アクティブ期間やスレッド数上限による）の対象になるかどうかを設定します。オフにすると、手動でのみアーカイブできます。
				</p>
				<div class="toggle-switch">
					<label class:disabled={isAutoArchiveLoading}>
						<input
							type="checkbox"
							bind:checked={autoArchiveEnabled}
							on:change={handleToggleAutoArchive}
							disabled={isAutoArchiveLoading}
						/>
						<span class="slider"></span>
						<span class="label-text">{autoArchiveEnabled ? '有効' : '無効'}</span>
					</label>
				</div>
				{#if autoArchiveMessage}
					<p class="message {autoArchiveMessage.type}">{autoArchiveMessage.text}</p>
				{/if}
			</div>
		{/if}
	</div>
</details>

<style>
	.settings-panel {
		background-color: #fef9e7;
		border: 1px solid #fbeebc;
		border-radius: 8px;
		margin-top: 1.5rem;
		margin-bottom: 1.5rem;
	}
	summary {
		font-weight: bold;
		cursor: pointer;
		padding: 1.5rem;
	}
	.settings-content-wrapper {
		padding: 0 1.5rem 1.5rem 1.5rem;
		flex-direction: column;
		gap: 2rem;
		display: flex;
	}
	.setting-item {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}
	h4 {
		margin: 0;
		color: #8d6e63;
	}
	.description {
		font-size: 0.9em;
		color: #666;
		margin: 0;
		line-height: 1.5;
	}
	form {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}
	.form-field {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}
	.form-group { display: flex; align-items: center; gap: 0.5rem; }
	label { font-weight: 500; }
	input, textarea { width: 100%; padding: 0.5rem; border: 1px solid #ccc; border-radius: 4px; font-size: 1em; }
	button { padding: 0.6rem 1.2rem; border-radius: 4px; border: none; background-color: #007bff; color: white; cursor: pointer; transition: background-color 0.2s; }
	button:hover { background-color: #e65100; }
	button:disabled { background-color: #ccc; cursor: not-allowed; }
	.radio-group {
		display: flex;
		gap: 1rem;
	}
	.radio-group label {
		display: flex;
		align-items: center;
		gap: 0.25rem;
		cursor: pointer;
		font-weight: normal;
		white-space: nowrap; /* テキストが折り返さないようにする */
	}
	.radio-group label.disabled {
		cursor: not-allowed;
		opacity: 0.7;
	}
	.form-actions {
		display: flex; justify-content: flex-end;
	}

	/* Toggle Switch Styles */
	.toggle-switch {
		display: flex;
		align-items: center;
	}
	.toggle-switch label {
		position: relative;
		display: inline-flex;
		align-items: center;
		cursor: pointer;
	}
	.toggle-switch input {
		opacity: 0;
		width: 0;
		height: 0;
	}
	.slider {
		position: relative;
		width: 50px;
		height: 26px;
		background-color: #ccc;
		border-radius: 26px;
		transition: background-color 0.2s;
	}
	.slider::before {
		position: absolute;
		content: '';
		height: 20px;
		width: 20px;
		left: 3px;
		bottom: 3px;
		background-color: white;
		border-radius: 50%;
		transition: transform 0.2s;
	}
	input:checked + .slider {
		background-color: #4caf50; /* Green for enabled */
	}
	input:checked + .slider::before {
		transform: translateX(24px);
	}
	.label-text {
		margin-left: 0.75rem;
		font-weight: 500;
	}

	.message {
		margin: 0;
		padding: 0.5rem 0.75rem;
		border-radius: 4px;
		font-size: 0.9em;
		width: 100%;
		box-sizing: border-box;
	}
	.message.success {
		background-color: #e8f5e9;
		color: #2e7d32;
		border: 1px solid #a5d6a7;
	}
	.message.error {
		background-color: #ffebee;
		color: #c62828;
		border: 1px solid #ef9a9a;
	}
</style>