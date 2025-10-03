<script lang="ts">
	// このコンポーネントが受け取るcommentオブジェクトの型を定義します。
	// バックエンドの`CommentResponse`と一致している必要があります。
	export let comment: {
		id: number;
		author_name: string | null;
		body: string;
		created_at: string;
		level: number | null; // levelはnullの場合があります
	};

	function formatDateTime(dateTimeString: string) {
		return new Date(dateTimeString).toLocaleString('ja-JP');
	}
</script>

<div class="comment-item">
	<div class="comment-header">
		<span class="author-name">{comment.author_name || '名無しさん'}</span>

		<!-- comment.levelがnullでない場合のみ、レベルバッジを表示します -->
		{#if comment.level !== null}
			<span class="level-badge">Lv.{comment.level}</span>
		{/if}

		<span class="timestamp">{formatDateTime(comment.created_at)}</span>
		<span class="comment-id">ID: {comment.id}</span>
	</div>
	<div class="comment-body">
		{@html comment.body}
	</div>
</div>

<style>
	.comment-item {
		border-top: 1px solid #eee;
		padding: 1rem 0;
	}
	.comment-header {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		font-size: 0.9rem;
		color: #555;
		margin-bottom: 0.5rem;
	}
	.author-name { font-weight: bold; color: #0056b3; }
	.level-badge { background-color: #e7f3ff; color: #0056b3; font-weight: bold; padding: 0.2rem 0.5rem; border-radius: 12px; font-size: 0.8rem; }
	.comment-body { white-space: pre-wrap; word-wrap: break-word; line-height: 1.6; }
</style>