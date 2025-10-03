<script lang="ts">
	import { createEventDispatcher } from 'svelte';
	import { page } from '$app/stores';
	import { invalidateAll } from '$app/navigation';

	export let isOpen = false;
	export let banTarget: {
		type: 'post' | 'comment' | 'user';
		id: number;
		banType?: 'User' | 'Ip' | 'Device'; // どのID部分からBANが開始されたかを受け取る
		// BANのスコープを決定するためのコンテキスト情報
		context?: {
			boardId?: number;
			postId?: number;
		};
	} | null = null;
	export let identityDetails: IdentityDetails | null = null;

	/** 板のモデレーション権限があるかどうか (管理者または板作成者) */
	export let canModerateBoard: boolean = false;

	type BanScope = 'Global' | 'Board' | 'Thread';

	// BANの種類は banTarget から直接取得する
	$: banType = banTarget?.banType ?? 'User';
	let scope: BanScope = 'Thread';
	let reason = '';
	let error: string | null = null;
	let isSubmitting = false;
	let isFetchingIdentity = false;

	// BAN対象の個人情報。モーダル表示時に取得する。
	let currentIdentityDetails: IdentityDetails | null = null;

	const dispatch = createEventDispatcher();

	$: isAdmin = $page.data.user?.role === 'admin';

	// モーダルが開かれたときに各種状態をリセットし、必要であれば個人情報を取得する
	$: if (isOpen && banTarget) {
		// スコープの初期値を設定
		if (banTarget.context?.postId) {
			scope = 'Thread';
		} else if (banTarget.context?.boardId) {
			scope = 'Board';
		} else {
			scope = 'Global';
		}

		// 個人情報の取得処理
		currentIdentityDetails = null;
		if (banTarget.type === 'user') {
			// ユーザーBANの場合、親から渡された情報をそのまま使う
			currentIdentityDetails = identityDetails;
		} else if (isAdmin) {
			// 投稿・コメントBANの場合、管理者は個人情報を取得する
			fetchIdentityDetails(banTarget.type, banTarget.id);
		}
	}

	async function fetchIdentityDetails(type: 'post' | 'comment', id: number) {
		isFetchingIdentity = true;
		error = null;
		try {
			const params = new URLSearchParams({ [type + '_id']: id.toString() });
			const res = await fetch(`/api/admin/identity-details?${params.toString()}`);
			if (!res.ok) {
				const errData = await res.json().catch(() => ({}));
				throw new Error(errData.error || `個人情報の取得に失敗 (HTTP ${res.status})`);
			}
			currentIdentityDetails = await res.json();
		} catch (e: any) {
			error = e.message;
		} finally {
			isFetchingIdentity = false;
		}
	}

	function closeModal(force = false) {
		if (isLoading && !force) return;
		// Reset form on close
		setTimeout(() => {
			currentIdentityDetails = null;
			reason = '';
			error = '';
		}, 300); // Wait for fade-out transition
		isOpen = false;
		dispatch('close');
	}

	$: isLoading = isSubmitting || isFetchingIdentity;

	async function handleSubmit() {
		if (!banTarget) return;
		isSubmitting = true;
		error = null;

		try {
			// ベースとなるリクエストボディを作成
			const body: Record<string, any> = {
				scope: scope,
				ban_type: banType,
				reason: reason.trim() || undefined,
				// 取得した個人情報を使用する
				source_email: currentIdentityDetails?.email || undefined,
				source_ip_address: currentIdentityDetails?.ip_address || undefined,
				source_device_info: currentIdentityDetails?.device_info || undefined
			};

			if (banTarget.type === 'user' && currentIdentityDetails) {
				// 管理者がユーザーを直接BANする場合、表示されているハッシュ値を直接使用する
				let hash_value: string | null | undefined;
				switch (banType) {
					case 'User':
						hash_value = currentIdentityDetails.permanent_user_hash;
						break;
					case 'Ip':
						hash_value = currentIdentityDetails.permanent_ip_hash;
						break;
					case 'Device':
						hash_value = currentIdentityDetails.permanent_device_hash;
						break;
					default:
						hash_value = null;
						break;
				}

				if (!hash_value) {
					throw new Error(`選択されたBANタイプ (${banType}) に対応するハッシュ値が見つかりません。`);
				}
				body.hash_value = hash_value;
			} else {
				// 投稿・コメントからのBAN (既存のロジック)
				body.post_id = banTarget.type === 'post' ? banTarget.id : undefined;
				body.comment_id = banTarget.type === 'comment' ? banTarget.id : undefined;
			}

			// スコープに応じて、ハッシュ直接指定の場合の板ID/スレIDを追加
			if (body.hash_value) {
				if (scope === 'Board') {
					body.board_id = banTarget?.context?.boardId;
				} else if (scope === 'Thread') {
					body.post_id = banTarget?.context?.postId;
				}
			}
			
			const response = await fetch('/api/bans', {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json'
				},
				body: JSON.stringify(body)
			});

			if (!response.ok) {
				const errorData = await response.json().catch(() => ({}));
				throw new Error(errorData.error || `BANの実行に失敗しました (HTTP ${response.status})`);
			}

			await invalidateAll(); // Refresh all data to reflect the ban
			closeModal(true); // BAN成功時は強制的に閉じる
		} catch (e: any) {
			error = e.message;
		} finally {
			isSubmitting = false;
		}
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			closeModal();
		}
	}
</script>

{#if isOpen}
	<div class="modal-overlay" on:click={closeModal} on:keydown={handleKeydown} role="dialog" aria-modal="true" tabindex="-1">
		<div class="modal-content" on:click|stopPropagation role="document">
			<button class="close-button" on:click={closeModal} aria-label="閉じる">&times;</button>
			<h2>BANを実行</h2>
			{#if banTarget?.type === 'user'}
				<p>対象: ユーザー ID: {banTarget?.id}</p>
			{:else}
				<p>対象: {banTarget?.type === 'post' ? '投稿' : 'コメント'} ID: {banTarget?.id}</p>
			{/if}

			<!-- Admin-only Identity Information -->
			{#if isAdmin && (isFetchingIdentity || currentIdentityDetails)}
				<div class="identity-info">
					<strong>個人情報 (管理者用)</strong>
					{#if isFetchingIdentity}
						<p>取得中...</p>
					{:else if currentIdentityDetails}
						<div class="identity-details">
							<p><strong>Email:</strong> {currentIdentityDetails.email || 'N/A'}</p>
							<p><strong>IP Address:</strong> {currentIdentityDetails.ip_address || 'N/A'}</p>
							<p><strong>Device Info:</strong> {currentIdentityDetails.device_info || 'N/A'}</p>
							<hr />
							<p><strong>User Hash:</strong> {currentIdentityDetails.permanent_user_hash || 'N/A'}</p>
							<p><strong>IP Hash:</strong> {currentIdentityDetails.permanent_ip_hash || 'N/A'}</p>
							<p><strong>Device Hash:</strong> {currentIdentityDetails.permanent_device_hash || 'N/A'}</p>
						</div>
					{/if}
				</div>
			{/if}

			<form on:submit|preventDefault={handleSubmit}>
				<div class="form-group">
					<label>BANの種類:</label>
					<p class="ban-type-display">
						{banType === 'User' ? 'ユーザーBAN' : banType === 'Ip' ? 'IP BAN' : 'デバイスBAN'}
					</p>
				</div>

				<div class="form-group">
					<label for="reason">理由 (任意):</label>
					<textarea id="reason" bind:value={reason} rows="3" maxlength="255" placeholder="BANの理由を入力"></textarea>
				</div>

				<div class="form-group">
					<label>適用範囲:</label>
					<div class="radio-group">
						{#if banTarget?.context?.postId}
							<label class="radio-label">
								<input type="radio" bind:group={scope} value="Thread" />
								スレッド内
							</label>
						{/if}
						{#if banTarget?.context?.boardId && (canModerateBoard || isAdmin)}
							<label class="radio-label">
								<input type="radio" bind:group={scope} value="Board" />
								板内
							</label>
						{/if}
						{#if isAdmin}
							<label class="radio-label">
								<input type="radio" bind:group={scope} value="Global" />
								グローバル (管理者のみ)
							</label>
						{/if}
					</div>
				</div>

				{#if error}
					<p class="error-message">{error}</p>
				{/if}

				<div class="form-actions">
					<button type="button" class="cancel-button" on:click={closeModal} disabled={isLoading}>キャンセル</button>
					<button type="submit" class="submit-button" disabled={isLoading}>
						{isSubmitting ? '実行中...' : 'BANを実行'}
					</button>
				</div>
			</form>
		</div>
	</div>
{/if}

<style>
	.modal-overlay {
		position: fixed;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		background-color: rgba(0, 0, 0, 0.6);
		display: flex;
		justify-content: center;
		align-items: center;
		z-index: 1000;
	}

	.modal-content {
		background-color: white;
		padding: 2rem;
		border-radius: 8px;
		box-shadow: 0 4px 15px rgba(0, 0, 0, 0.2);
		width: 90%;
		max-width: 500px;
		position: relative;
		max-height: 90vh;
		overflow-y: auto;
	}

	.close-button {
		position: absolute;
		top: 10px;
		right: 10px;
		background: none;
		border: none;
		font-size: 1.5rem;
		cursor: pointer;
	}

	h2 {
		margin-top: 0;
		margin-bottom: 1.5rem;
	}

	.form-group {
		margin-bottom: 1rem;
	}

	label {
		display: block;
		margin-bottom: 0.5rem;
	}

	.ban-type-display {
		font-weight: bold;
		margin: 0;
		padding: 0.5rem;
		background-color: #f0f2f5;
		border-radius: 4px;
	}

	.radio-group {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.radio-label {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-weight: normal;
	}

	select,
	textarea {
		width: 100%;
		padding: 0.5rem;
		border: 1px solid #ccc;
		border-radius: 4px;
		font-size: 1rem;
	}

	.form-actions {
		display: flex;
		justify-content: flex-end;
		gap: 0.5rem;
		margin-top: 1.5rem;
	}

	.submit-button,
	.cancel-button {
		padding: 0.6rem 1.2rem;
		border-radius: 4px;
		border: none;
		font-weight: 500;
	}

	.submit-button {
		background-color: #dc3545;
		color: white;
	}
	.submit-button:disabled {
		background-color: #f8d7da;
	}

	.cancel-button {
		background-color: #f0f0f0;
	}

	.error-message {
		color: #dc3545;
		margin-top: 1rem;
	}

	.identity-info {
		background-color: #fffbe6;
		border: 1px solid #ffe58f;
		border-radius: 4px;
		padding: 1rem;
		margin-bottom: 1.5rem;
		font-size: 0.9rem;
		text-align: left;
	}

	.identity-details {
		margin-top: 0.5rem;
		font-size: 0.8rem;
		word-break: break-all;
	}

	.identity-details hr {
		border: none;
		border-top: 1px solid #ffe58f;
		margin: 0.5rem 0;
	}
</style>
