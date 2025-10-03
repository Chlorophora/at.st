<script lang="ts">
	import type { PageData } from './$types';
	import { goto, invalidateAll } from '$app/navigation';
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import Pagination from '$lib/components/Pagination.svelte';

	export let data: PageData;

	// フォームの状態を管理する変数
	let sort: string;
	let q: string;
	let searchField: string;
	let searchType: string;
	let includeAuthorNames: boolean;
	let includeActiveThreads: boolean;
	let boardId: string;
	let createdYear: string;
	let createdMonth: string;
	let minResponses: string;
	let showDeleted = false;

	$: isAdmin = $page.data.user?.role === 'admin';
	let isMounted = false; // マウントされたかを追跡するフラグ

	// ページネーション関連
	$: totalPages = data && data.totalCount !== undefined && data.limit !== undefined ? Math.ceil(data.totalCount / data.limit) : 1; // Defensive check

	// 並び順が変更されたら自動的に検索をトリガー
	// マウント後のユーザー操作による変更のみを検知する
	$: if (isMounted && sort) {
		handleSearchAndSort();
	}

	// ページ読み込み時にフォームの状態をURLから復元
	// 初回アクセス時は新着過去ログ一覧が表示されるように、ソートは固定
	onMount(() => {
		sort = data.searchParams?.sort || 'archived_at_desc';
		q = data.searchParams?.q || '';
		searchField = data.searchParams?.searchField || 'title';
		searchType = data.searchParams?.searchType || 'and';
		includeAuthorNames = data.searchParams?.includeAuthorNames || false;
		// デフォルトでは現行スレッドを含める (true)
		includeActiveThreads = data.searchParams?.includeActiveThreads ?? true;
		boardId = data.searchParams?.boardId || '';
		createdYear = data.searchParams?.createdYear || '';
		createdMonth = data.searchParams?.createdMonth || '';
		minResponses = data.searchParams?.minResponses || '';
		showDeleted = data.searchParams?.show_deleted || false;
		isMounted = true; // マウント完了
	});

	const handleSearchAndSort = () => {
		const params = new URLSearchParams();
		if (sort) params.append('sort', sort);
		if (q) {
			params.append('q', q);
			if (searchField) params.append('search_field', searchField);
			if (searchType) params.append('search_type', searchType);
			if (searchField === 'body' && includeAuthorNames) {
				params.append('include_author_names', 'true');
			}
		}
		if (boardId) params.append('board_id', boardId);
		if (createdYear) params.append('created_year', createdYear);
		if (createdMonth) params.append('created_month', createdMonth);
		if (minResponses) params.append('min_responses', minResponses);
		// チェックが外れている場合 (false の場合) のみパラメータを追加
		if (!includeActiveThreads) {
			params.append('include_active_threads', 'false');
		}
		// 管理者がチェックした場合のみパラメータを追加
		if (isAdmin && showDeleted) {
			params.append('show_deleted', 'true');
		}
		// 検索・ソート時は1ページ目に戻る
		goto(`/archive?${params.toString()}`);
	};

	const changePage = (page: number) => {
		const params = new URLSearchParams(window.location.search);
		params.set('page', page.toString());
		goto(`/archive?${params.toString()}`);
	};

	async function handleRestore(postId: number) {
		if (!confirm(`ID: ${postId} のスレッドを復元しますか？`)) {
			return;
		}
		try {
			const res = await fetch(`/api/posts/${postId}/restore`, {
				method: 'POST',
				credentials: 'include'
			});
			if (res.ok) {
				alert('スレッドを復元しました。');
				// データを再読み込みしてリストを更新
				await invalidateAll();
			} else {
				const error = await res.json().catch(() => ({ error: '復元に失敗しました。' }));
				alert(`復元に失敗しました: ${error.error || res.statusText}`);
			}
		} catch (e) {
			console.error('Restore failed', e);
			alert('復元中にエラーが発生しました。');
		}
	}
</script>

<div class="container">
	<h1>過去ログ倉庫</h1>

	<form on:submit|preventDefault={handleSearchAndSort} class="controls">
		<div class="form-row">
			<div class="search-group">
				<input type="text" bind:value={q} placeholder="検索キーワード" />
				<select bind:value={searchField}>
					<option value="title">タイトル</option>
					<option value="body">本文</option>
				</select>
				<label>
					<input type="radio" bind:group={searchType} value="and" /> AND
				</label>
				<label>
					<input type="radio" bind:group={searchType} value="or" /> OR
				</label>
				{#if searchField === 'body'}
					<label>
						<input type="checkbox" bind:checked={includeAuthorNames} />
						名前も含める
					</label>
				{/if}
			</div>
			<div class="sort-group">
				<label>
					並び順:
					<select bind:value={sort}>
						<option value="archived_at_desc">新着アーカイブ順</option>
						<option value="created_at_desc">新着作成順</option>
						<option value="created_at_asc">古い作成順</option>
					</select>
				</label>
				<label>
					板ID:
					<input type="text" bind:value={boardId} placeholder="複数可" />
				</label>
				<label>
					<input type="checkbox" bind:checked={includeActiveThreads} />
					現行スレッドを含める
				</label>
			</div>
		</div>
		<div class="form-row">
			<div class="filter-group">
				<label>
					作成年:
					<input type="number" bind:value={createdYear} placeholder="西暦" min="2025" max="2100" />
				</label>
				<label>
					作成月:
					<select bind:value={createdMonth}>
						<option value="">-- 月 --</option>
						{#each Array(12) as _, i}
							<option value={i + 1}>{i + 1}月</option>
						{/each}
					</select>
				</label>
				<label>
					レス数:
					<input type="number" bind:value={minResponses} placeholder="以上" min="1" />
				</label>
			</div>
			<div class="form-actions">
				<button type="submit">絞り込み</button>
			</div>
		</div>
		{#if isAdmin}
			<div class="form-row admin-controls">
				<label>
					<input type="checkbox" bind:checked={showDeleted} on:change={handleSearchAndSort} />
					削除済みスレッドを表示
				</label>
			</div>
		{/if}
	</form>

	{#if data.error}
		<p class="error">{data.error}</p>
	{:else if !data.archivedPosts || data.archivedPosts.length === 0}
		<p>条件に一致する過去ログはありません。</p>
	{:else}
		<p>{data.totalCount}件の過去ログが見つかりました。</p>

		<!-- 上部のページネーション -->
		<Pagination currentPage={data.currentPage} {totalPages} on:change={(e) => changePage(e.detail)} />
		<div class="post-list">
			{#each data.archivedPosts as post (post.id)}
				<div
					class="post-item"
					class:active-thread={!post.archived_at && !post.deleted_at}
					class:deleted-thread={!!post.deleted_at}
				>
					<h2 class="post-title">
						<a class="title-link" href={`/posts/${post.id}`}>{post.title}</a>
						<span class="responses-count">レス: {post.total_responses}</span>
					</h2>
					<div class="meta">
						{#if post.board_name}
							<span>板: {post.board_name} (#{post.board_id})</span>
						{:else}
							<span>板ID: {post.board_id || 'N/A'}</span>
						{/if}
						<span>名前: {post.author_name || '名無し'}</span>
						<span>作成: {new Date(post.created_at).toLocaleString()}</span>
						{#if post.deleted_at}
							<span class="deleted-at-label">削除: {new Date(post.deleted_at).toLocaleString()}</span>
						{:else if post.archived_at}
							<span>アーカイブ: {new Date(post.archived_at).toLocaleString()}</span>
						{:else}
							<span class="active-thread-label">現行スレッド</span>
						{/if}
					</div>
					<p class="post-body" title={post.body.replace(/<[^>]*>?/gm, '')}>{@html post.body}</p>
					{#if post.deleted_at && isAdmin}
						<div class="post-actions">
							<button class="restore-button" on:click={() => handleRestore(post.id)}>復元</button>
						</div>
					{/if}
				</div>
			{/each}
		</div>

		<!-- 下部のページネーション -->
		<Pagination currentPage={data.currentPage} {totalPages} on:change={(e) => changePage(e.detail)} />
	{/if}
</div>

<style>
	.container {
		max-width: 960px;
		margin: 2rem auto;
		padding: 1rem;
	}
	.controls {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		padding: 1.5rem;
		background-color: #f7f7f7;
		border: 1px solid #e0e0e0;
		border-radius: 12px;
		margin-bottom: 2rem;
	}
	.form-row {
		display: flex;
		flex-wrap: wrap;
		gap: 1rem;
		align-items: center;
	}
	.search-group,
	.sort-group,
	.filter-group {
		display: flex;
		flex-wrap: wrap;
		gap: 1rem;
		align-items: center;
	}
	.controls label {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}
	.controls input,
	.controls select {
		padding: 0.5rem 0.75rem;
		font-size: 0.9rem;
		border-radius: 6px;
		border: 1px solid #ccc;
	}
	.form-actions {
		margin-left: auto; /* 2行目の右端にボタンを配置 */
	}
	.controls button[type='submit'] {
		padding: 0.75rem 1.5rem;
		font-size: 1rem;
		font-weight: bold;
		border-radius: 8px;
		border: none;
		background-color: #2d8cff;
		color: white;
		cursor: pointer;
		transition: background-color 0.2s;
	}
	.controls button[type='submit']:hover {
		background-color: #0070e0;
	}

	.post-list {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}
	.post-item {
		border: 1px solid #e0e0e0;
		padding: 0.75rem 1rem;
		border-radius: 6px;
		transition: background-color 0.2s, border-color 0.2s;
	}
	.post-item.active-thread {
		background-color: #e7f5ff; /* A light blue to indicate it's active */
		border-left: 4px solid #2d8cff;
		padding-left: calc(1rem - 4px); /* Keep inner padding consistent */
	}
	.post-item.deleted-thread {
		background-color: #fff0f1;
		border-color: #f5c6cb;
		border-left: 4px solid #dc3545;
		padding-left: calc(1rem - 4px);
	}
	.post-item:hover {
		background-color: #fcfcfc;
		border-color: #ccc;
	}
	.post-title {
		margin: 0;
		font-size: 1.1rem;
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 1rem;
	}
	.title-link {
		text-decoration: none;
		color: #0056b3;
		font-weight: 600;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.title-link:hover {
		text-decoration: underline;
	}
	.responses-count {
		font-size: 0.9rem;
		font-weight: normal;
		color: #333;
		background-color: #e9e9e9;
		padding: 0.1rem 0.5rem;
		border-radius: 10px;
		flex-shrink: 0; /* 縮まないようにする */
	}
	.meta {
		font-size: 0.9rem; /* 文字サイズを本文と統一 */
		color: #777;
		display: flex;
		flex-wrap: wrap;
		gap: 0.25rem 1rem;
		margin-top: 0.5rem; /* 上の余白を本文と統一 */
	}
	.active-thread-label {
		font-weight: bold;
		color: #1e88e5;
		background-color: rgba(45, 140, 255, 0.1);
		padding: 0.1rem 0.5rem;
		border-radius: 10px;
	}
	.deleted-at-label {
		font-weight: bold;
		color: #dc3545;
		background-color: rgba(220, 53, 69, 0.1);
		padding: 0.1rem 0.5rem;
	}
	.post-body {
		font-size: 0.9rem;
		color: #555;
		margin-top: 0.5rem;
		margin-bottom: 0.25rem; /* pタグのデフォルトマージンをリセット */
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		max-width: 100%;
	}
	.post-actions {
		margin-top: 0.75rem;
		text-align: right;
	}
	.restore-button {
		padding: 0.4rem 0.8rem;
		font-size: 0.9rem;
		color: white;
		background-color: #28a745;
		border: none;
		border-radius: 4px;
		cursor: pointer;
		transition: background-color 0.2s;
		flex-shrink: 0;
	}
	.restore-button:hover {
		background-color: #218838;
	}
	.admin-controls {
		width: 100%;
	}
	.error { color: red; }
</style>

<!--
[PROMPT_SUGGESTION]フロントエンドのページネーションに、特定のページ番号に直接ジャンプする機能を追加してください。[/PROMPT_SUGGESTION]
[PROMPT_SUGGESTION]APIのレスポンスに、総ページ数も計算して含めるように変更してください。[/PROMPT_SUGGESTION]
-->