<script lang="ts">
	import type { PageData } from './$types';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores'; // isAdminを導出するためにpageストアをインポート
	import BanModal from '$lib/components/BanModal.svelte';
	import AdminBoardSettings from '$lib/components/AdminBoardSettings.svelte';
	import { onDestroy, onMount } from 'svelte';
	import { fly } from 'svelte/transition';
	import { openThreadFormPanel, closeThreadFormPanel } from '$lib/stores/ui';
	import ThreadFormPanel from '$lib/components/ThreadFormPanel.svelte';
	import { decodeHtmlEntities } from '$lib/utils/html';
	import type { IdentityDetails } from 'src/app';

	export let data: PageData;

	onMount(() => {
		// ページのURLを正規化します。
		// 末尾にスラッシュがない場合、ブラウザの履歴を書き換えてスラッシュを追加します。
		// これにより、サーバーサイドでのリダイレクトループを回避しつつ、URLを統一できます。
		const { pathname, search } = window.location;
		if (pathname.length > 1 && !pathname.endsWith('/')) {
			history.replaceState(history.state, '', `${pathname}/${search}`);
		}
	});

	// ソート順を管理するリアクティブな変数
	let sortOrder = $page.url.searchParams.get('sort') || 'momentum_desc';

	// ソート順の選択肢
	const sortOptions = [
		{ value: 'momentum_desc', label: '勢い (降順)' },
		{ value: 'momentum_asc', label: '勢い (昇順)' },
		{ value: 'responses_desc', label: 'レス数 (降順)' },
		{ value: 'responses_asc', label: 'レス数 (昇順)' },
		{ value: 'last_activity_desc', label: '最終更新 (降順)' },
		{ value: 'last_activity_asc', label: '最終更新 (昇順)' },
		{ value: 'created_at_desc', label: '作成日時 (降順)' },
		{ value: 'created_at_asc', label: '作成日時 (昇順)' }
	];

	// ログインユーザーが管理者かどうかを判定するリアクティブな変数
	$: isAdmin = $page.data.user?.role === 'admin';
	$: showFloatingButton = data.user && data.board && !data.board.archived_at;

	// ソートされたスレッド一覧
	$: sortedPosts = data.posts
		? [...data.posts].sort((a, b) => {
				const [key, direction] = sortOrder.split('_');
				const dir = direction === 'asc' ? 1 : -1;

				let valA, valB;

				switch (key) {
					case 'momentum':
						valA = a.momentum ?? 0;
						valB = b.momentum ?? 0;
						break;
					case 'responses':
						valA = a.response_count ?? 0;
						valB = b.response_count ?? 0;
						break;
					case 'last_activity':
						valA = new Date(a.last_activity_at).getTime();
						valB = new Date(b.last_activity_at).getTime();
						break;
					case 'created_at':
						valA = new Date(a.created_at).getTime();
						valB = new Date(b.created_at).getTime();
						break;
					default:
						return 0;
				}

				return (valA < valB ? -1 : valA > valB ? 1 : 0) * dir;
			})
		: [];

	onDestroy(() => {
		// ユーザーがこのページから離れるときに、スレッド作成パネルを確実に閉じます。
		closeThreadFormPanel();
	});

	// BANモーダル関連のstate
	let isBanModalOpen = false;
	let banTarget: {
		type: 'post' | 'comment' | 'user';
		id: number;
		banType?: 'User' | 'Ip' | 'Device';
		context?: {
			boardId?: number;
			postId?: number;
		};
	} | null = null;
	let identityDetails: IdentityDetails | null = null;

	// 管理者設定が更新されたときにUIに即時反映させるためのハンドラ
	function handleBoardUpdate(event: CustomEvent<Board>) {
		data.board = { ...data.board, ...event.detail };
	}

	// アーカイブ状態をトグルする関数
	async function toggleArchiveStatus() {
		if (!data.board) return;

		const isArchived = !!data.board.archived_at;
		const endpoint = `/api/admin/boards/${data.board.id}/${isArchived ? 'unarchive' : 'archive'}`;

		try {
			const response = await fetch(endpoint, {
				method: 'POST'
			});

			if (!response.ok) {
				const errorData = await response.json();
				throw new Error(errorData.error || '操作に失敗しました。');
			}

			// 成功したらページを再読み込みして状態を更新
			// invalidateAll()でも良いが、表示全体が変わるためリロードが確実
			window.location.reload();

		} catch (error) {
			alert(`エラー: ${error.message}`);
		}
	}

	// 日付を整形するヘルパー関数
	function formatDateTime(isoString: string): string {
		if (!isoString) return '';
		const date = new Date(isoString);
		// APIからUTCで渡されるため、JSTで表示するようにタイムゾーンを指定
		return date.toLocaleString('ja-JP', { timeZone: 'Asia/Tokyo' });
	}

	// 日付を相対時間で整形するヘルパー関数
	function formatRelativeTime(isoString: string): string {
		if (!isoString) return '';
		// 常に現在時刻（JST）との差を計算
		const now = new Date();
		// APIから来る時刻はUTCなので、JSTに変換
		const past = new Date(isoString);
		const pastJST = new Date(past.toLocaleString('en-US', { timeZone: 'Asia/Tokyo' }));
		
		const diffInSeconds = Math.floor((now.getTime() - pastJST.getTime()) / 1000);

		if (diffInSeconds < 60) {
			return 'たった今';
		}
		const minutes = Math.floor(diffInSeconds / 60);
		if (minutes < 60) {
			return `${minutes}分前`;
		}
		const hours = Math.floor(minutes / 60);
		if (hours < 24) {
			return `${hours}時間前`;
		}
		const days = Math.floor(hours / 24);
		return `${days}日前`;
	}

	// 勢いを整形するヘルパー関数
	function formatMomentum(momentum: number | null | undefined): string {
		return (momentum ?? 0).toFixed(2);
	}

	// 板作成者用のBANモーダルを開く関数
	async function openBanModalForCreatorIdPart(banType: 'User' | 'Ip' | 'Device') {
		// このUIは管理者のみに表示されるため、ここでの権限チェックは不要
		if (!data.board?.created_by) return;

		banTarget = {
			type: 'user',
			id: data.board.created_by,
			context: { boardId: data.board.id },
			banType
		};
		identityDetails = null; // 以前の情報をクリア
		isBanModalOpen = true;

		try {
			const response = await fetch(`/api/admin/identity-details?user_id=${data.board.created_by}`);
			if (response.ok) {
				identityDetails = await response.json();
			}
		} catch (error) {
			console.error('板作成者の個人情報の取得中にエラーが発生しました:', error);
		}
	}
</script>

<svelte:head>
	{#if data.board}
		<title>{data.board.name}</title>
	{:else if data.error}
		<title>エラー: {data.error}</title>
	{:else}
		<title>板を読み込んでいます...</title>
	{/if}
</svelte:head>

{#if data.error}
	<p class="error">{data.error}</p>
{:else if data.board}
	<h1>{data.board.name}</h1>
	<p class="board-description"><span class="board-id">(#{data.board.id})</span> {data.board.description}</p>

	{#if data.board.archived_at}
		<div class="archived-banner">
			<p>この板はアーカイブされています。新しいスレッドの作成はできません。</p>
		</div>
	{/if}

	<!-- 管理者向け: 板作成者情報 -->
	{#if isAdmin && data.creatorInfo}
		<div class="creator-info-admin">
			<span>板作成者:</span>
			<span>{formatDateTime(data.board.created_at)}</span>
			<span>
				{#if data.creatorInfo.level_at_creation !== null && data.creatorInfo.level_at_creation !== data.creatorInfo.level}
					Lv.{data.creatorInfo.level_at_creation}↝Lv.{data.creatorInfo.level}
				{:else}
					Lv.{data.creatorInfo.level}
				{/if}
			</span>
			<span class="response-id">
				ID:
				{#each data.creatorInfo.display_user_id.split('-') as part, i}
					{@const type = i === 0 ? 'User' : i === 1 ? 'Ip' : 'Device'}
					<span
						class="id-part"
						class:bannable={isAdmin && type !== 'Device'}
						on:click={() => openBanModalForCreatorIdPart(type)}
						on:keydown={(e) => e.key === 'Enter' && openBanModalForCreatorIdPart(type)}
						role="button"
						tabindex="0"
					>{part}</span>{#if i < 2}-{/if}
				{/each}
			</span>
		</div>
	{/if}

	<!-- 管理者・作成者向け: 板設定パネル -->
	{#if data.board.can_moderate}
		<div class="admin-controls">
			<AdminBoardSettings board={data.board} on:update={handleBoardUpdate} />
			<!-- アーカイブ/解除ボタンは管理者のみに表示 -->
			{#if isAdmin}
				<div class="archive-control">
					<button on:click={toggleArchiveStatus} class:archived={!!data.board.archived_at}>
						{data.board.archived_at ? 'アーカイブ解除' : '板をアーカイブする'}
					</button>
				</div>
			{/if}
		</div>
	{/if}

	<hr />

	<div class="list-header">
		<h2>スレッド一覧</h2>
		<div class="sort-container">
			<label for="sort-order">表示順:</label>
			<select
				id="sort-order"
				bind:value={sortOrder}
				on:change={() => goto(`?sort=${sortOrder}`, { keepFocus: true, noScroll: true })}
			>
				{#each sortOptions as option}
					<option value={option.value}>{option.label}</option>
				{/each}
			</select>
		</div>
	</div>

	{#if sortedPosts.length > 0}
		<ul class="post-list">
			{#each sortedPosts as post}
				<li class="post-list-item">
				<a href="/posts/{post.id}" class="post-link-container">
					<div class="post-info">
						<div class="post-title-wrapper">
								<span class="post-title">{decodeHtmlEntities(post.title)}</span>
						</div>
						<div class="post-meta">
							<small class="post-timestamp created-at"
								><span class="meta-label">作成:&nbsp;</span>{formatDateTime(
									post.created_at
								)}</small
							>
							<span class="post-momentum"
								><span class="meta-label">勢い:&nbsp;</span>{formatMomentum(post.momentum)}</span
							>
							<span class="post-responses">
								{#if post.response_count !== undefined && post.response_count !== null}
									<span class="meta-label">レス数:&nbsp;</span>{post.response_count}
								{/if}
							</span>
							<small class="post-timestamp" title={formatDateTime(post.last_activity_at)}>
								<span class="meta-label">更新:&nbsp;</span>{formatRelativeTime(
									post.last_activity_at
								)}
							</small>
						</div>
						</div>
				</a>
				</li>
			{/each}
		</ul>
	{:else}
		<p>この板にはまだスレッドがありません。</p>
	{/if}

	<hr class="form-separator" />

{:else}
	<p>板の情報を読み込み中です...</p>
{/if}

<!-- BANモーダルコンポーネントを配置 -->
<BanModal bind:isOpen={isBanModalOpen} {banTarget} {identityDetails} />

{#if data.board}
	<ThreadFormPanel boardId={data.board.id} />
{/if}

{#if showFloatingButton}
	<button
		class="floating-action-button"
		title="新しいスレッドを投稿"
		on:click|preventDefault={openThreadFormPanel}
		transition:fly={{ y: 100, duration: 300 }}
	>
		<!-- ペンアイコン -->
		<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
			<path d="M12 20h9" />
			<path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" />
		</svg>
	</button>
{/if}

<style>
	.archived-banner {
		background-color: #ffebee;
		color: #c62828;
		border: 1px solid #ef9a9a;
		border-radius: 8px;
		padding: 0.5rem 1.5rem;
		margin: 1rem 0;
	}
	.creator-info-admin {
		background-color: #fef9e7;
		border: 1px solid #fbeebc;
		border-radius: 4px;
		padding: 0.75rem;
		margin-top: 1rem;
		font-size: 0.9em;
		display: flex;
		flex-wrap: wrap;
		gap: 0.5em 1em;
		align-items: center;
	}
	.admin-controls {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}
	.archive-control button {
		background-color: #f44336;
	}
	.archive-control button.archived {
		background-color: #4caf50;
	}
	form.disabled {
		opacity: 0.6;
	}
	.id-part.bannable {
		cursor: pointer;
		color: #007bff;
	}
	.id-part.bannable:hover {
		color: #0056b3;
	}
	.list-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 1rem;
	}
	.list-header h2 {
		margin: 0;
	}
	.sort-container {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}
	.sort-container label {
		font-weight: 500;
		font-size: 0.9em;
	}
	.sort-container select {
		padding: 0.6rem 1rem; /* サイズを大きくする */
		border: 1px solid #ccc;
		border-radius: 4px;
		font-size: 1em; /* フォントサイズも少し大きくする */
	}
	.board-id {
		font-style: normal;
	}
	.board-description {
		font-style: italic;
		color: #666;
	}
	.post-list {
		list-style: none;
		padding: 0;
		border: 1px solid #ddd; /* 枠線の色を濃くする */
		border-radius: 4px; /* 角丸もulに移動 */
	}
	.post-list-item {
		/* border: 1px solid #eee; */ /* 個別の枠線を削除 */
		padding: 1rem;
		/* margin-bottom: 1rem; */ /* 枠間のマージンを削除 */
		/* border-radius: 4px; */ /* 個別の角丸を削除 */
		transition: background-color 0.2s ease;
	}
	.post-list-item + .post-list-item {
		border-top: 1px solid #ddd; /* 区切り線の色も合わせる */
	}
	.post-list-item:hover {
		background-color: #f9f9f9;
	}
	a.post-link-container {
		display: block;
		text-decoration: none;
		color: inherit;
	}
	.post-info {
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
		width: 100%;
	}
	.post-title-wrapper {
		display: flex;
		align-items: baseline;
		gap: 0.5rem;
		/* タイトルが長い場合に折り返すようにする */
		overflow: hidden;
	}
	.post-title {
		font-weight: 500;
		font-size: 1.1rem; /* タイトルを少し大きく */
	}
	a.post-link-container:hover .post-title {
		text-decoration: underline;
	}
	.post-meta {
		display: grid;
		grid-template-columns: 10em 7em 7em auto;
		align-items: baseline;
		gap: 0.8rem;
		color: #555;
		font-size: 0.85em;
	}
	.post-responses,
	.post-momentum {
		font-weight: bold;
		white-space: nowrap;
	}
	.post-timestamp {
		white-space: nowrap;
	}
	.error {
		color: red;
		font-weight: bold;
	}

	.form-separator {
		margin: 2rem 0;
	}

	.floating-action-button {
		position: fixed;
		bottom: 2rem;
		right: 2rem;
		width: 56px;
		height: 56px;
		background-color: #2d8cff;
		color: white;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
		z-index: 1005; /* ヘッダー(1000)より手前、パネル(1010)より後ろ */
		text-decoration: none;
		border: none; /* button要素のデフォルトの枠線をリセット */
		cursor: pointer;
		transition: background-color 0.2s ease, transform 0.2s ease;
	}

	.floating-action-button:hover {
		background-color: #0070e0;
		transform: translateY(-2px);
	}

	.error-message {
		color: red;
		margin-top: 0.5rem;
		white-space: pre-wrap; /* 改行を反映 */
	}
	.field-error {
		font-size: 0.9em;
		margin-top: 0.25rem;
	}

	/* スマートフォン向けのメタ情報ラベル非表示 */
	@media (max-width: 768px) {
		.meta-label {
			display: none;
		}
		.floating-action-button {
			bottom: 1.5rem;
			right: 1.5rem;
		}
		.post-list {
			border: none;
			border-radius: 0;
			background-color: transparent; /* 背景色もリセット */
			/* 親要素(main)のパディング(0.75rem)を相殺して画面幅いっぱいに広げる */
			margin-left: -0.75rem;
			margin-right: -0.75rem;
		}
		.post-list li {
			/* 左右のパディングをなくし、境界線を画面幅いっぱいにする */
			padding-left: 0;
			padding-right: 0;
		}
		.post-list-item a.post-link-container {
			/* なくしたパディングをリンク要素に適用し、コンテンツの余白を維持 */
			padding-left: 1rem;
			padding-right: 1rem;
		}
		.post-meta {
			display: flex; /* GridレイアウトからFlexboxに変更 */
			flex-wrap: wrap; /* 画面幅に応じて折り返すようにする */
			gap: 0.2rem 0.8rem; /* 縦横の隙間を調整して詰める */
			grid-template-columns: auto; /* gridの列定義をリセット */
		}
	}
</style>
