<script lang="ts">
	import type { PageData } from './$types';
	import HistoryItemDisplay from '$lib/components/HistoryItemDisplay.svelte';

	export let data: PageData;

	// フォームの状態を管理するための、書き込み可能なローカル変数。
	let user_part: string;
	let ip_part: string;
	let device_part: string;
	let logic: string;
	let sort: string;

	// 検索結果やエラーメッセージは、`data`から直接リアクティブに派生させます。
	$: historyResponse = data.historyResponse;
	$: searchError = data.searchError;

	// `data`プロパティが変更されたとき（例：ブラウザの戻る/進むボタンでのナビゲーション）にのみ、
	// フォームのローカル変数を更新します。この書き方により、リアクティブな依存関係が `data` オブジェクトのみに限定され、
	// ユーザーがフォームを操作してローカル変数が変更されても、このブロックが再実行されて入力が上書きされるのを防ぎます。
	$: ((d) => {
		if (!d) return;
		user_part = d.user_part || '';
		ip_part = d.ip_part || '';
		device_part = d.device_part || '';

		// 検索結果がある場合（＝検索後）はURLのパラメータを反映し、
		// ない場合（＝初回表示）はデフォルト値を強制的に設定します。
		if (d.historyResponse) {
			logic = d.logic || 'and';
			sort = d.sort || 'thread_desc';
		} else {
			logic = 'and';
			sort = 'thread_desc';
		}
	})(data);

	// 「スレッド順」でソートされている場合に、結果をスレッドごとにグループ化するためのリアクティブな処理
	$: groupedItems = (() => {
		// スレッド順でない、または結果がない場合はグループ化しない (nullを返す)
		if (!historyResponse?.items || !sort.startsWith('thread_')) {
			return null;
		}

		const groups = new Map<number, { title: string | null; items: App.HistoryItem[]; post_id: number }>();

		// バックエンドから送られてきたソート順を維持したままグループ化する
		for (const item of historyResponse.items) {
			const threadId = item.type === 'Post' ? item.data.id : item.data.post_id;
			const threadTitle = item.type === 'Post' ? item.data.title : item.data.post_title;

			if (!groups.has(threadId)) {
				groups.set(threadId, { title: threadTitle, items: [], post_id: threadId });
			}
			groups.get(threadId)!.items.push(item);
		}
		// Mapから値の配列を返すことで、スレッドの表示順を維持する
		return Array.from(groups.values());
	})();

	// スレッドごとの投稿数を計算するためのリアクティブな処理
	$: threadContributionCounts = (() => {
		if (!historyResponse?.items) {
			return new Map<number, number>();
		}
		const counts = new Map<number, number>();
		for (const item of historyResponse.items) {
			const threadId = item.type === 'Post' ? item.data.id : item.data.post_id;
			counts.set(threadId, (counts.get(threadId) || 0) + 1);
		}
		return counts;
	})();

	function handlePaste(event: ClipboardEvent) {
		const pastedText = event.clipboardData?.getData('text/plain');
		if (!pastedText) return;

		// "ID: 01aaa948-cc4f1036-6a1c4b36" のような形式に対応
		const cleanedText = pastedText.trim().replace(/^ID:\s*/i, '');
		const parts = cleanedText.split('-');

		// ハイフンで3分割できたら、それぞれの入力欄に値をセット
		if (parts.length === 3) {
			event.preventDefault(); // デフォルトのペースト動作をキャンセル
			user_part = parts[0] || '';
			ip_part = parts[1] || '';
			device_part = parts[2] || '';
		}
	}
</script>

<svelte:head>
	<title>必死チェッカー</title>
	<meta name="description" content="投稿IDの一部から投稿履歴を検索します。" />
</svelte:head>

<div class="container">
	<h1>必死チェッカー仮</h1>
	<p>投稿IDを入力して、関連する投稿履歴を検索します</p>

	<form method="GET" class="search-form">
		<div class="form-group id-search-group">
			<label for="user_part">ID:</label>
			<div class="id-input-group">
				<div class="input-with-clear">
					<input type="text" id="user_part" name="user_part" bind:value={user_part} on:paste={handlePaste} placeholder="" maxlength="8" />
					{#if user_part}
						<button type="button" class="clear-btn" on:click={() => (user_part = '')} title="クリア">
							<svg width="17" height="17" viewBox="0 0 12 12" fill="none" xmlns="http://www.w3.org/2000/svg">
								<path d="M1 11L11 1" stroke="currentColor" stroke-width="1.2" />
								<path d="M1 1L11 11" stroke="currentColor" stroke-width="1.2" />
							</svg>
						</button>
					{/if}
				</div>
				<span>-</span>
				<div class="input-with-clear">
					<input type="text" id="ip_part" name="ip_part" bind:value={ip_part} on:paste={handlePaste} placeholder="" maxlength="4" />
					{#if ip_part}
						<button type="button" class="clear-btn" on:click={() => (ip_part = '')} title="クリア">
							<svg width="17" height="17" viewBox="0 0 12 12" fill="none" xmlns="http://www.w3.org/2000/svg">
								<path d="M1 11L11 1" stroke="currentColor" stroke-width="1.2" />
								<path d="M1 1L11 11" stroke="currentColor" stroke-width="1.2" />
							</svg>
						</button>
					{/if}
				</div>
				<span>-</span>
				<div class="input-with-clear">
					<input type="text" id="device_part" name="device_part" bind:value={device_part} on:paste={handlePaste} placeholder="" maxlength="4" />
					{#if device_part}
						<button type="button" class="clear-btn" on:click={() => (device_part = '')} title="クリア">
							<svg width="17" height="17" viewBox="0 0 12 12" fill="none" xmlns="http://www.w3.org/2000/svg">
								<path d="M1 11L11 1" stroke="currentColor" stroke-width="1.2" />
								<path d="M1 1L11 11" stroke="currentColor" stroke-width="1.2" />
							</svg>
						</button>
					{/if}
				</div>
			</div>
			{#if user_part || ip_part || device_part}
				<button type="button" class="clear-all-btn" on:click={() => { user_part = ''; ip_part = ''; device_part = ''; }} title="すべてクリア">
					<svg width="17" height="17" viewBox="0 0 12 12" fill="none" xmlns="http://www.w3.org/2000/svg">
						<path d="M1 11L11 1" stroke="currentColor" stroke-width="1.2" />
						<path d="M1 1L11 11" stroke="currentColor" stroke-width="1.2" />
					</svg>
				</button>
			{/if}
		</div>

		<div class="form-group form-group-inline">
			<div class="radio-group">
				<label><input type="radio" name="logic" value="and" bind:group={logic} />AND</label>
				<label><input type="radio" name="logic" value="or" bind:group={logic} />OR</label>
			</div>
			<select id="sort" name="sort" bind:value={sort}>
				<option value="thread_desc">スレッド (新しい順)</option>
				<option value="thread_asc">スレッド (古い順)</option>
				<option value="time_desc">投稿日時 (新しい順)</option>
				<option value="time_asc">投稿日時 (古い順)</option>
			</select>
		</div>

		<button type="submit">検索</button>
	</form>

	{#if searchError}
		<div class="error-message">
			<p>{searchError}</p>
		</div>
	{/if}

	{#if historyResponse}
		<div class="results-container">
			<h2>検索結果</h2>

			<div class="summary-card">
				<h3>概要</h3>
				<p><strong>総投稿数:</strong> {historyResponse.summary.total_contribution_count}</p>
				<p><strong>スレッド作成数:</strong> {historyResponse.summary.created_thread_count}</p>
				<p>
					<strong>初回投稿:</strong>
					{historyResponse.summary.first_seen
						? new Date(historyResponse.summary.first_seen).toLocaleString()
						: 'N/A'}
				</p>
				<p>
					<strong>最終投稿:</strong>
					{historyResponse.summary.last_seen
						? new Date(historyResponse.summary.last_seen).toLocaleString()
						: 'N/A'}
				</p>

				<h4>作成したスレッド ({historyResponse.summary.created_threads.length}件)</h4>
				<ul>
					{#each historyResponse.summary.created_threads as [title, count]}
						<li>{title} ({count})</li>
					{/each}
				</ul>

				<h4>コメントしたスレッド ({historyResponse.summary.commented_in_threads.length}件)</h4>
				<ul>
					{#each historyResponse.summary.commented_in_threads as [title, count]}
						<li>{title} ({count})</li>
					{/each}
				</ul>
			</div>

			<h3>投稿一覧</h3>

			{#if groupedItems}
				<!-- スレッド順でソートされている場合の表示 -->
				<div class="grouped-items-list">
					{#each groupedItems as group}
						<section class="thread-group">
							<h4 class="thread-group-header">
								<a href="/posts/{group.post_id}">
									{group.title || `(タイトル不明 ID: ${group.post_id})`}
									<span class="contribution-count">({threadContributionCounts.get(group.post_id) || 0})</span>
								</a>
							</h4>
							<div class="items-list">
								{#each group.items as item (item.type + '_' + item.data.id)}
									<HistoryItemDisplay
										{item}
										resNum={item.type === 'Post' ? 1 : item.data.response_number}
										showThreadTitle={false}
										bordered={false}
									/>
								{/each}
							</div>
						</section>
					{/each}
				</div>
			{:else}
				<!-- 時間順でソートされている場合の表示 -->
				<div class="items-list">
					{#each historyResponse.items as item (item.type + '_' + item.data.id)}
						{@const threadId = item.type === 'Post' ? item.data.id : item.data.post_id}
						<HistoryItemDisplay
							{item}
							resNum={item.type === 'Post' ? 1 : item.data.response_number}
							contributionCount={threadContributionCounts.get(threadId)}
						/>
					{/each}
				</div>
			{/if}
		</div>
	{/if}
</div>

<style>
	.container {
		max-width: 800px;
		margin: 2rem auto;
		padding: 1rem;
	}
	.search-form {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		border: 1px solid #ccc;
		padding: 1.5rem;
		border-radius: 8px;
		margin-bottom: 2rem;
		background-color: #f9f9f9;
	}
	.form-group {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}
	.form-group-inline {
		flex-direction: row;
		justify-content: space-between;
		align-items: center;
		flex-wrap: wrap;
		gap: 1.5rem;
	}
	.radio-group {
		display: flex;
		gap: 1.5rem;
		flex-wrap: wrap;
	}
	.form-group.id-search-group {
		flex-direction: row;
		align-items: center;
		gap: 0.75rem;
	}
	.id-input-group {
		display: flex;
		align-items: center;
		gap: 0.25rem;
		flex-grow: 1;
	}
	.input-with-clear {
		position: relative;
		display: flex;
		flex: 1;
		align-items: center;
	}
	.input-with-clear input {
		/* ボタンのスペースを確保 */
		padding-right: 2rem;
		width: 100%;
		flex: 1;
		text-align: center;
		font-family: monospace;
	}
	.id-input-group span {
		color: #888;
	}
	.clear-btn {
		position: absolute;
		right: 0.5rem;
		top: 50%;
		transform: translateY(-50%);
		border: none;
		background: transparent;
		cursor: pointer;
		color: #999;
		line-height: 1;
		padding: 0;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.clear-btn:hover {
		color: #333;
	}
	.clear-all-btn {
		border: none;
		background: transparent;
		cursor: pointer;
		color: #888;
		line-height: 1;
		padding: 0 0.25rem;
		margin-left: 0.25rem;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.clear-all-btn:hover {
		color: #333;
	}
	.radio-group label {
		font-weight: normal;
	}
	.form-group label {
		font-weight: bold;
	}
	.form-group input[type='text'],
	.form-group select {
		padding: 0.5rem;
		border: 1px solid #ccc;
		border-radius: 4px;
		font-size: 1rem;
	}
	button[type='submit'] {
		padding: 0.75rem;
		background-color: #007bff;
		color: white;
		border: none;
		border-radius: 4px;
		font-size: 1.1rem;
		cursor: pointer;
		transition: background-color 0.2s;
	}
	button[type='submit']:hover {
		background-color: #0056b3;
	}
	.error-message {
		background-color: #f8d7da;
		color: #721c24;
		border: 1px solid #f5c6cb;
		padding: 1rem;
		border-radius: 5px;
		margin: 1rem 0;
	}
	.results-container {
		margin-top: 2rem;
	}
	.summary-card {
		background-color: #eef7ff;
		border: 1px solid #bde0ff;
		padding: 1.5rem;
		border-radius: 8px;
		margin-bottom: 2rem;
	}
	.summary-card h3,
	.summary-card h4 {
		margin-top: 0;
		border-bottom: 1px solid #bde0ff;
		padding-bottom: 0.5rem;
		margin-bottom: 1rem;
	}
	.summary-card ul {
		padding-left: 20px;
		margin-top: 0.5rem;
	}
	.summary-card li {
		margin-bottom: 0.25rem;
	}
	.grouped-items-list {
		display: flex;
		flex-direction: column;
		gap: 2rem; /* スレッドグループ間のスペースを広めに取る */
	}
	.thread-group {
		border: 1px solid #ddd;
		border-radius: 8px;
		background-color: #fff;
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
	}
	.thread-group-header {
		/* グループのカード内に収まるように調整 */
		padding: 1rem 1.5rem;
		margin: 0; /* h4タグが持つデフォルトの上下マージンをリセット */
		border-bottom: 2px solid #007bff;
	}
	.thread-group-header a {
		text-decoration: none;
		color: #333;
		font-size: 1.2rem;
	}

	.contribution-count {
		margin-left: 0.5em;
		font-weight: normal;
	}

	/* グループ内のアイテムリストのスタイル */
	.thread-group .items-list {
		gap: 0; /* アイテム間のgapをなくし、ボーダーで区切る */
	}
	.thread-group .items-list > :global(.history-item-card:not(:last-child)) {
		border-bottom: 1px solid #eee;
	}

	.items-list {
		display: flex;
		flex-direction: column;
		gap: 1.5rem;
	}
</style>
