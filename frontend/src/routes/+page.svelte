<script lang="ts">
	import type { PageData } from './$types';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import Pagination from '$lib/components/Pagination.svelte';

	export let data: PageData;

	// 板検索用の検索クエリ
	let boardNameQuery = '';
	let boardIdQuery = '';

	// ログインユーザーが管理者かどうかを判定
	$: isAdmin = data.user?.role === 'admin';

	const BOARDS_PER_PAGE = 100;

	// ここでトップに固定表示したい板の情報を設定します
	const featuredBoards = [
		{
			id: 1, // 表示したい板のID
			name: '紅茶', // 表示する板の名前
			description: '実況チャンネル' // 表示する説明
		},
		{
			id: 2, // 例: 表示したいもう一つの板のID
			name: 'なんU', // 表示する板の名前
			description: 'なんでも雑談可' // 表示する説明
		}
	];

	// URLから現在のページ番号を取得し、総ページ数を計算
	$: currentPage = parseInt($page.url.searchParams.get('page') || '1');
	$: totalPages = data.paginatedBoards ? Math.ceil(data.paginatedBoards.total_count / BOARDS_PER_PAGE) : 1;

	// 検索クエリに基づいて表示中の板一覧をフィルタリングする
	$: filteredBoards =
		data.paginatedBoards?.items
			.filter((board) => {
				// 管理者でない場合、アーカイブ済みの板を非表示にする
				if (!isAdmin && board.archived_at) {
					return false;
				}
				return true;
			})
			.filter((board) => {
				// 検索ボックスによる絞り込み
				const nameQuery = boardNameQuery.trim().toLowerCase();
				const idQuery = boardIdQuery.trim();

				const nameMatch = nameQuery ? board.name.toLowerCase().includes(nameQuery) : true;
				const idMatch = idQuery ? board.id.toString() === idQuery : true; // IDは完全一致

				return nameMatch && idMatch;
			}) || [];

	// ページネーションコンポーネントからのイベントを処理
	function handlePageChange(event: CustomEvent<number>) {
		const newPage = event.detail;
		goto(`/?page=${newPage}`);
	}
</script>

<h1>☕掲示板</h1>

{#if !data.user}
	<div class="announcement">
		<a href="/auth/register">掲示板へ書き込むためには認証が必要です</a>
	</div>
{/if}
<div class="announcement">
	<a href="/blog/kiji">この掲示板について</a>
</div>

<div class="featured-boards-container">
	{#each featuredBoards as board}
		<a href="/boards/{board.id}" class="featured-board-link">
			<strong>{board.name}</strong>
			<p>{board.description}</p>
		</a>
	{/each}
</div>

{#if data.paginatedBoards && data.paginatedBoards.items.length > 0}
	<h2>板一覧</h2>

	<div class="search-container">
		<div class="search-group">
			<div class="input-with-clear">
				<input
					type="search"
					id="board-name-search"
					bind:value={boardNameQuery}
					placeholder="板名検索"
					class="board-search-input"
				/>
				{#if boardNameQuery}
					<button type="button" class="clear-btn" on:click={() => (boardNameQuery = '')} title="クリア">
						<svg width="17" height="17" viewBox="0 0 12 12" fill="none" xmlns="http://www.w3.org/2000/svg">
							<path d="M1 11L11 1" stroke="currentColor" stroke-width="1.2" />
							<path d="M1 1L11 11" stroke="currentColor" stroke-width="1.2" />
						</svg>
					</button>
				{/if}
			</div>
		</div>
		<div class="search-group">
			<div class="input-with-clear">
				<input
					type="search"
					id="board-id-search"
					bind:value={boardIdQuery}
					placeholder="板IDを入力"
					class="board-search-input"
				/>
				{#if boardIdQuery}
					<button type="button" class="clear-btn" on:click={() => (boardIdQuery = '')} title="クリア">
						<svg width="17" height="17" viewBox="0 0 12 12" fill="none" xmlns="http://www.w3.org/2000/svg">
							<path d="M1 11L11 1" stroke="currentColor" stroke-width="1.2" />
							<path d="M1 1L11 11" stroke="currentColor" stroke-width="1.2" />
						</svg>
					</button>
				{/if}
			</div>
		</div>
	</div>

	{#if filteredBoards.length > 0}
		<ul class="board-list">
			{#each filteredBoards as board}
				<li class:archived={!!board.archived_at && isAdmin}>
					<a href="/boards/{board.id}">
						<strong>
							{board.name}
							{#if isAdmin && board.archived_at}
								<span class="archived-tag">[アーカイブ済み]</span>
							{/if}
						</strong>
						<p>(#{board.id}) {board.description}</p>
					</a>
				</li>
			{/each}
		</ul>
	{:else}
		<p>検索条件に一致する板は見つかりませんでした。</p>
	{/if}
	<!-- 検索クエリがなく、総ページ数が1より大きい場合のみページネーションを表示 -->
	{#if !boardNameQuery.trim() && !boardIdQuery.trim() && totalPages > 1}
		<Pagination {currentPage} {totalPages} on:change={handlePageChange} />
	{/if}
{:else}
	<p>現在、利用できる板はありません。</p>
{/if}

<style>
	.announcement {
		background-color: #fdf8f2;
		border: 1px solid #eaddc7;
		border-left: 5px solid #a56a43;
		padding: 1rem;
		margin-bottom: 1rem;
		border-radius: 4px;
	}

	.announcement a {
		display: block;
		text-decoration: none;
		color: #333;
		font-weight: 600;
	}
	
	.announcement a:hover {
		text-decoration: underline;
	}

	.featured-boards-container {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		margin-bottom: 2rem;
	}

	.featured-board-link {
		/* aタグ自体をカードにする */
		display: block;
		padding: 1rem;
		border: 1px solid #007bff;
		background-color: #f0f8ff;
		border-radius: 8px;
		text-decoration: none;
		color: inherit;
		transition: background-color 0.2s ease;
		height: 100%;
		box-sizing: border-box;
	}
	.featured-board-link:hover {
		background-color: #e0f0ff;
	}

	.search-container {
		display: flex;
		gap: 1.5rem;
		margin-bottom: 1.5rem;
	}

	.search-group {
		flex: 1;
		display: flex;
		flex-direction: column;
	}

	.input-with-clear {
		position: relative;
		display: flex;
		align-items: center;
	}

	.search-group label {
		margin-bottom: 0.5rem;
		font-weight: bold;
		font-size: 0.9em;
	}

	.board-search-input {
		width: 100%;
		padding: 0.75rem;
		font-size: 1rem;
		border: 1px solid #ccc;
		border-radius: 4px;
		box-sizing: border-box;
		padding-right: 2.5rem; /* クリアボタン用のスペース */
	}

	/* ブラウザネイティブの検索フィールドのクリアボタンを非表示にする */
	.board-search-input::-webkit-search-decoration,
	.board-search-input::-webkit-search-cancel-button,
	.board-search-input::-webkit-search-results-button,
	.board-search-input::-webkit-search-results-decoration {
		display: none;
	}

	.clear-btn {
		position: absolute;
		right: 0.75rem;
		top: 50%;
		transform: translateY(-50%);
		background: transparent;
		border: none;
		padding: 0;
		cursor: pointer;
		color: #6c757d;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.clear-btn:hover {
		color: #343a40;
	}

	hr {
		margin: 2rem 0;
		border: 0;
		border-top: 1px solid #eee;
	}
	.error {
		color: red;
		font-weight: bold;
	}
	.board-list {
		list-style: none;
		padding: 0;
	}
	.board-list li a {
		display: block;
		padding: 1rem;
		border: 1px solid #ccc;
		margin-bottom: 0.5rem;
		text-decoration: none;
		color: inherit;
		border-radius: 4px;
	}
	.board-list li a:hover {
		background-color: #f0f0f0;
	}
	.featured-board-link p,
	.board-list p {
		margin: 0.25rem 0 0;
		font-size: 0.9em;
		color: #555;
	}

	.board-list li.archived {
		background-color: #f5f5f5;
		opacity: 0.8;
		border-color: #e0e0e0;
	}

	.archived-tag {
		color: #c62828;
		font-size: 0.85em;
		font-weight: normal;
		margin-left: 0.5rem;
		vertical-align: middle;
	}

	/* --- スマートフォン向けのスタイル --- */
	@media (max-width: 768px) {
		h1 {
			font-size: 1.5rem;
		}

		h2 {
			font-size: 1.25rem;
		}

		.featured-board-link,
		.board-list li a {
			/* カード内の余白を詰める */
			padding: 0.75rem;
		}
	}
</style>
