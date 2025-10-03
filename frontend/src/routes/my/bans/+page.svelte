<script lang="ts">
	import type { PageData } from './$types';
	import { invalidateAll, goto } from '$app/navigation';
	import Pagination from '$lib/components/Pagination.svelte';

	export let data: PageData;

	let error: string | null = null;
	let isLoading: { [key: number]: boolean } = {};

	async function deleteBan(banId: number) {
		if (!confirm(`ID: ${banId} のBANを本当に解除しますか？`)) {
			return;
		}
		isLoading[banId] = true;
		error = null;

		try {
			// hooks.server.tsのプロキシ設定を活用するため、相対パスを使用します。
			// これにより、APIのURLを一元管理でき、CORSの問題も回避できます。
			const response = await fetch(`/api/bans/${banId}`, {
				method: 'DELETE'
			});

			if (!response.ok) {
				const errorData = await response.json().catch(() => ({}));
				throw new Error(errorData.error || `BANの解除に失敗しました (HTTP ${response.status})`);
			}

			// ページデータを再読み込みして一覧を更新
			await invalidateAll();
		} catch (e: any) {
			error = e.message;
		} finally {
			isLoading[banId] = false;
		}
	}

	function formatDateTime(dateTimeString: string | null) {
		if (!dateTimeString) return '無期限';
		return new Date(dateTimeString).toLocaleString('ja-JP');
	}

	// Pagination
	$: totalPages = data.totalCount ? Math.ceil(data.totalCount / data.limit) : 1;

	function changePage(page: number) {
		const params = new URLSearchParams(window.location.search);
		params.set('page', page.toString());
		goto(`/my/bans?${params.toString()}`, {
			keepFocus: true, // ページ遷移後もフォーカスを維持
			noScroll: true // ページ上部へのスクロールを抑制
		});
	}
</script>

<svelte:head>
	<title>BAN一覧</title>
</svelte:head>

<div class="user-bans-container">
	<h1>BANの管理画面</h1>

	{#if error}
		<p class="error-message">{error}</p>
	{/if}

	{#if data.bans && data.bans.length > 0}
		<div class="info-bar">
			<p>全{data.totalCount}件</p>
			<Pagination currentPage={data.currentPage} {totalPages} on:change={(e) => changePage(e.detail)} />
		</div>
	{/if}

	<div class="table-wrapper">
		<table>
			<thead>
				<tr>
					<th>ID</th>
					<th>種類</th>
					<th>対象ハッシュ</th>
					<th>スコープ</th>
					<th>対象</th>
					<th>理由</th>
					<th>日時</th>
					<th>期限</th>
					<th>操作</th>
				</tr>
			</thead>
			<tbody>
				{#each data.bans as ban (ban.id)}
					<tr>
						<td>{ban.id}</td>
						<td>{ban.ban_type}</td>
						<td class="hash-cell">{ban.hash_value}</td>
						<td>
							<span class="scope-{ban.scope.toLowerCase()}">{ban.scope_display_name}</span>
						</td>
						<td>
							{#if ban.scope === 'Thread'}
								<a href="/posts/{ban.post_id}" target="_blank" rel="noopener noreferrer">{ban.post_title || 'N/A'}</a>
							{:else if ban.scope === 'Board'}
								<a href="/boards/{ban.board_id}" target="_blank" rel="noopener noreferrer">{ban.board_name || 'N/A'}</a>
							{:else}
								-
							{/if}
						</td>
						<td>{ban.reason || 'N/A'}</td>
						<td>{formatDateTime(ban.created_at)}</td>
						<td>{formatDateTime(ban.expires_at)}</td>
						<td>
							<button
								class="action-button delete-button"
								on:click={() => deleteBan(ban.id)}
								disabled={isLoading[ban.id]}
							>
								{isLoading[ban.id] ? '解除中...' : '解除'}
							</button>
						</td>
					</tr>
				{:else}
					<tr>
						<td colspan="9" style="text-align: center;">あなたが作成した有効なBANはありません。</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>

	{#if data.bans && data.bans.length > 0 && totalPages > 1}
		<div class="pagination-bottom">
			<Pagination currentPage={data.currentPage} {totalPages} on:change={(e) => changePage(e.detail)} />
		</div>
	{/if}
</div>

<style>
	.user-bans-container { padding: 2rem; max-width: 1200px; margin: 0 auto; }
	.table-wrapper { overflow-x: auto; }
	table { width: 100%; border-collapse: collapse; font-size: 0.9rem; }
	th, td { border: 1px solid #ddd; padding: 0.75rem; text-align: left; vertical-align: top; }
	thead { background-color: #f2f2f2; }
	.hash-cell { word-break: break-all; font-family: monospace; font-size: 0.8rem; max-width: 200px; }
	.scope-global {
		font-weight: bold;
		color: #c0392b;
	}
	.scope-board {
		font-weight: bold;
		color: #d35400;
	}
	.scope-thread {
		font-weight: bold;
		color: #2980b9;
	}
	.action-button { padding: 0.4rem 0.8rem; border-radius: 4px; border: 1px solid #ccc; background-color: #fff; cursor: pointer; }
	.delete-button { background-color: #f8d7da; border-color: #f5c6cb; color: #721c24; }
	.error-message { color: #dc3545; background-color: #f8d7da; border: 1px solid #f5c6cb; padding: 1rem; border-radius: 4px; margin-bottom: 1rem; }
	.info-bar {
		display: flex;
		justify-content: space-between;
		align-items: center;
		flex-wrap: wrap;
		gap: 1rem;
		margin-bottom: 1.5rem;
	}
	.info-bar p {
		margin: 0;
	}
	.pagination-bottom {
		margin-top: 1.5rem;
		display: flex;
		justify-content: center;
	}
</style>