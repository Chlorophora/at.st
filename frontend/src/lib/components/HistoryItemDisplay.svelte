<script lang="ts">
	export let item: HistoryItem;
	export let resNum: number | undefined;
	export let showThreadTitle: boolean = true;
	export let bordered: boolean = true;
	export let contributionCount: number | undefined = undefined;

	function formatDateTime(dateString: string) {
		return new Date(dateString).toLocaleString('ja-JP');
	}

	/**
	 * レベル表示用の文字列を生成します。
	 * @param entity Post または Comment オブジェクト
	 * @returns 表示用のレベル文字列 (例: "Lv.3", "Lv.3↝Lv.10")、または表示不要な場合は null
	 */
	function getLevelDisplay(entity: Post | Comment | null | undefined): string | null {
		if (entity?.level_at_creation == null || entity.level_at_creation === '') {
			return null;
		}

		const levelAtCreation = Number(entity.level_at_creation);
		const currentLevel = entity.level == null || entity.level === '' ? null : Number(entity.level);

		if (currentLevel !== null && currentLevel !== levelAtCreation) {
			return `Lv.${levelAtCreation}↝Lv.${currentLevel}`;
		}

		return `Lv.${levelAtCreation}`;
	}

	$: levelDisplay = getLevelDisplay(item.data);
</script>

<div class="history-item-card" class:bordered>
	{#if showThreadTitle}
		<div class="item-header">
			<!-- 投稿タイプ表示 -->
			{#if item.type === 'Post'}
				<span class="item-type post">スレ建て</span>
			{:else}
				<span class="item-type comment">レス</span>
			{/if}

			<!-- スレッドタイトル -->
			<h4 class="item-title">
				{#if item.type === 'Post'}
					<a href="/posts/{item.data.id}">{item.data.title}</a>
				{:else}
					<a href="/posts/{item.data.post_id}#comment-{item.data.id}">
						{item.data.post_title || `スレッド: ${item.data.post_id}`}
					</a>
				{/if}
				{#if contributionCount !== undefined}
					<span class="contribution-count">({contributionCount})</span>
				{/if}
			</h4>
		</div>
	{/if}

	<div class="response-header">
		{#if resNum}
			<span class="response-number">{resNum}:</span>
		{/if}
		<span class="response-author">{item.data.author_name || '名無し'}</span>
		<span class="response-timestamp">{formatDateTime(item.data.created_at)}</span>
		{#if levelDisplay}
			<span>{levelDisplay}</span>
		{/if}
		{#if item.data.display_user_id}
			<span class="response-id">ID: {item.data.display_user_id}</span>
		{/if}
	</div>

	<div class="item-body">
		<p>{@html item.data.body.replace(/\n/g, '<br>')}</p>
	</div>
</div>

<style>
	.history-item-card {
		padding: 1rem 1.5rem;
		display: flex;
		flex-direction: column;
		gap: 0.5rem; /* 各セクション間のスペース */
	}

	.history-item-card.bordered {
		border: 1px solid #ddd;
		border-radius: 8px;
		background-color: #fff;
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
	}

	.item-header {
		display: flex;
		align-items: baseline; /* テキストのベースラインを揃える */
		gap: 1em; /* 各要素間のスペース */
		/* スレッドタイトルとレス情報の間に区切り線を入れる */
		border-bottom: 1px solid #eee;
		padding-bottom: 0.75rem;
	}

	.response-header {
		/* /posts/[id] のスタイルを適用 */
		color: #666;
		font-size: 0.9em;
		display: flex;
		flex-wrap: wrap;
		gap: 0.25em 0.5em; /* 字間をスレッド詳細ページに合わせて狭くする */
		align-items: baseline;
	}

	.item-type {
		font-weight: bold;
		padding: 0.2rem 0.5rem;
		border-radius: 3px;
		color: white;
		font-size: 0.8em;
	}
	.item-type.post {
		background-color: #007bff;
	}
	.item-type.comment {
		background-color: #28a745;
	}

	.item-title {
		margin: 0;
		font-size: 1.1rem;
		font-weight: bold;
	}

	.contribution-count {
		margin-left: 0.5em;
		font-weight: normal;
		color: #555;
	}

	.item-title a {
		text-decoration: none;
		color: #005a9c;
	}
	.item-title a:hover {
		text-decoration: underline;
	}

	.response-header .response-number {
		font-weight: bold;
	}

	.response-author {
		font-weight: bold;
		color: #007bff;
	}

	.response-id {
		font-family: inherit; /* monospaceを解除してスレッド詳細ページと合わせる */
	}

	.item-body {
		line-height: 1.6;
		white-space: pre-wrap;
		word-break: break-all;
	}
	.item-body p {
		margin: 0;
	}
</style>
