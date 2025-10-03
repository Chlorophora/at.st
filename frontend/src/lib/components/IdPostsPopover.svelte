<script lang="ts">
	import type { Post, Comment } from 'src/app';
	import { createEventDispatcher } from 'svelte';
	import { fixDoubleEscaping } from '$lib/utils/html';
	import { zIndexManager } from '$lib/stores/zIndex';
	import { clickOutside } from '$lib/actions/clickOutside';

	export let id: string;
	export let banType: 'User' | 'Ip' | 'Device';
	export let idPartValue: string;
	export let posts: (Post | Comment)[];
	export let sourceId: number;
	export let canModerate: boolean;
	export let backlinkCounts: Map<number, number>;
	export let isAdmin: boolean = false;
	export let position: { x: number; y: number };
	export let asModalContent = false;

	const dispatch = createEventDispatcher();

	// コンポーネント作成時にz-indexを取得して、マウント時から最前面に表示されるようにする
	let zIndex: number | undefined = asModalContent ? undefined : zIndexManager.getNewZIndex();

	function bringToFront() {
		if (!asModalContent) {
			zIndex = zIndexManager.getNewZIndex();
		}
	}

	$: filteredItems = posts
		.map((p, i) => {
			if (!p || !p.display_user_id) return null;
			const parts = p.display_user_id.split('-');
			if (parts.length !== 3) return null;

			let match = false;
			if (banType === 'User' && parts[0] === idPartValue) match = true;
			if (banType === 'Ip' && parts[1] === idPartValue) match = true;
			if (banType === 'Device' && parts[2] === idPartValue) match = true;

			if (match) {
				return { response: p, number: i + 1 };
			}
			return null;
		})
		.filter((item): item is { response: Post | Comment; number: number } => item !== null);

	$: totalCount = filteredItems.length;
	$: sourceIndex = filteredItems.findIndex((item) => item.response.id === sourceId) + 1;

	// BANボタンの表示条件をより厳密にする
	$: showBanButton = canModerate && filteredItems.length > 0 && (banType !== 'Device' || isAdmin);

	function handleBanClick() {
		// ポップアップを開くきっかけとなった元の投稿/コメントを `posts` 配列から探す
		const originalItem = posts.find((p) => p.id === sourceId);

		if (!originalItem) {
			// 通常は発生しないが、念のため
			console.error('BANの元となる投稿が見つかりませんでした。sourceId:', sourceId);
			// ユーザーにフィードバックが必要な場合は、エラー表示処理を追加
			return;
		}

		const sourceType = 'post_id' in originalItem ? 'comment' : 'post';

		dispatch('openbanmodal', {
			sourceType,
			sourceId: originalItem.id, // 元の投稿のIDを渡す
			banType // どのID部分（User/Ip/Device）がクリックされたかを渡す
		});
		dispatch('close'); // BANモーダルを開いたら、このポップアップは閉じます
	}

	function handleItemClick(responseNumber: number) {
		const targetElement = document.getElementById(`res-${responseNumber}`);
		if (targetElement) {
			targetElement.scrollIntoView({ behavior: 'smooth', block: 'start' });
		}
		// スクロールが開始されるのを待ってからポップアップを閉じます
		setTimeout(() => {
			dispatch('close');
		}, 500);
	}

	function handleIdPartClick(
		e: MouseEvent | KeyboardEvent,
		responseId: number,
		newBanType: 'User' | 'Ip' | 'Device',
		newIdPartValue: string
	) {
		e.stopPropagation();
		dispatch('showidposts', {
			banType: newBanType,
			idPartValue: newIdPartValue,
			event: e,
			sourceId: responseId
		});
	}

	function handleResponseNumberClick(e: MouseEvent | KeyboardEvent, responseNumber: number) {
		e.stopPropagation(); // .response-item のクリックイベントを抑制
		if (!backlinkCounts?.has(responseNumber)) return;
		dispatch('showbacklinks', {
			responseNumber: responseNumber,
			event: e,
			level: 0 // IdPostsPopoverからのBacklinksは常にレベル0
		});
	}

	function handleBodyClick(e: MouseEvent) {
		const anchor = (e.target as HTMLElement).closest('a.response-anchor');
		if (anchor) {
			// e.stopImmediatePropagation(); // response-item のクリックイベント(handleItemClick)を抑制
			e.preventDefault(); // デフォルトのアンカー挙動（ページ内ジャンプ）を抑制
			const href = anchor.getAttribute('href');
			const match = href?.match(/#res-(\d+)/);
			if (match?.[1]) {
				dispatch('showresponse', {
					responseNumber: parseInt(match[1], 10),
					event: e 
				});
			}
		}
	}

	function formatDateTime(isoString: string): string {
		const date = new Date(isoString);
		return date.toLocaleString('ja-JP');
	}

	// Helper to get level display
	function getLevelDisplay(entity: Post | Comment | null | undefined): string | null {
		if (entity?.level_at_creation == null || entity.level_at_creation === '') {
			return null;
		}
		const levelAtCreation = Number(entity.level_at_creation);
		const currentLevel =
			entity.level == null || entity.level === '' ? null : Number(entity.level);
		if (currentLevel !== null && currentLevel !== levelAtCreation) {
			return `Lv.${levelAtCreation}↝Lv.${currentLevel}`;
		}
		return `Lv.${levelAtCreation}`;
	}
</script>

<script context="module" lang="ts">
	function linkifyUrls(text: string): string {
		if (!text) return '';
		const urlRegex = /(https?:\/\/[^\s<>"']+)/g;
		return text.replace(urlRegex, (url) => `<a href="${url}" target="_blank" rel="noopener noreferrer" class="external-link">${url}</a>`);
	}
</script>

{#if asModalContent || position}
	<div
		{id}
		class:id-posts-popover={!asModalContent}
		class:modal-view={asModalContent}
		style:left={!asModalContent && position ? `${position.x}px` : undefined}
		style:top={!asModalContent && position ? `${position.y}px` : undefined}
		style:z-index={zIndex}
		on:mousedown={bringToFront}
		use:clickOutside on:outclick={() => dispatch('close')}
	>
		<header class="popover-header">
			<h3 class="header-title">
				<span>ID: {idPartValue}</span>
				{#if totalCount > 0 && sourceIndex > 0}
					<span class="count-display">({sourceIndex}/{totalCount})</span>
				{/if}
			</h3>
			{#if showBanButton}
				<button class="ban-button" on:click={handleBanClick}>このIDをBAN</button>
			{/if}
		</header>
		<div class="popover-content">
			{#if filteredItems.length > 0}
				{#each filteredItems as { response, number } (number)}
					{@const levelDisplay = getLevelDisplay(response)}
					<article
						class="response-item"
						on:click={(e) => {
							// レスアンカーのクリックでない場合のみ記事全体のクリックとして扱う
							if (!(e.target as HTMLElement).closest('a.response-anchor')) {
								handleItemClick(number);
							}
						}}
						on:keydown={(e) => e.key === 'Enter' && handleItemClick(number)}
						role="button"
						tabindex="0"
					>
						<header class="response-header">
							{#if backlinkCounts?.get(number)}
								<span
									class="response-number clickable-backlinks"
									on:click|stopPropagation={(e) => handleResponseNumberClick(e, number)}
									on:keydown|stopPropagation={(e) => e.key === 'Enter' && handleResponseNumberClick(e, number)}
									role="button"
									tabindex="0"
								>
									{number}: ({backlinkCounts.get(number)})
								</span>
							{:else}
								<span class="response-number">{number}:</span>
							{/if}
							<span class="response-author">{response.author_name || '野球民'}</span>
							<span class="response-timestamp">{formatDateTime(response.created_at)}</span>
							{#if levelDisplay}
								<span>{levelDisplay}</span>
							{/if}
							{#if response.display_user_id}
								<span class="response-id">
									ID:
									{#each response.display_user_id.split('-') as part, i}
										{@const type = i === 0 ? 'User' : i === 1 ? 'Ip' : 'Device'}
										<span
											class:highlight={type === banType}
											class:bannable={canModerate && type !== 'Device'}
											on:click|stopPropagation={(e) => handleIdPartClick(e, response.id, type, part)}
											on:keydown|stopPropagation={(e) => e.key === 'Enter' && handleIdPartClick(e, response.id, type, part)}
											role="button"
											tabindex="0"
										>
											{part}
										</span>{#if i < 2}-{/if}
									{/each}
								</span>
							{/if}
						</header>
						<div class="response-body" on:click={handleBodyClick}>
							{@html linkifyUrls(fixDoubleEscaping(response.body))}
						</div>
					</article>
				{/each}
			{:else}
				<p class="no-results">このIDを持つ投稿はスレッド内に見つかりませんでした。</p>
			{/if}
		</div>
	</div>
{/if}

<style>
	/* スタイルは BacklinksPopover.svelte や ResponsePopover.svelte を参考にしています */
	.modal-view {
		padding: 0.5rem 0.75rem;
	}

	.id-posts-popover {
		position: fixed;
		background-color: #ffffff;
		border: 1px solid #ccc;
		border-radius: 6px;
		box-shadow: 0 5px 15px rgba(0, 0, 0, 0.2);
		padding: 0.75rem;
		font-size: 0.9rem;
		min-width: 300px;
		max-width: 500px;
		max-height: 70vh;
		display: flex;
		flex-direction: column;
	}

	.popover-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding-bottom: 0.5rem;
		margin-bottom: 0.5rem;
		border-bottom: 1px solid #eee;
	}

	.header-title {
		font-weight: bold;
		margin: 0;
		font-size: 1em;
		display: flex;
		align-items: baseline;
		gap: 0.5em;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.count-display {
		font-weight: normal;
		font-size: 0.9em;
		color: #666;
		flex-shrink: 0;
	}

	.ban-button {
		padding: 0.25rem 0.6rem;
		border-radius: 4px;
		border: 1px solid #dc3545;
		background-color: #f8d7da;
		color: #721c24;
		font-size: 0.85em;
		font-weight: 500;
		cursor: pointer;
		flex-shrink: 0;
	}
	.ban-button:hover {
		background-color: #dc3545;
		color: white;
	}

	.popover-content {
		overflow-y: auto;
	}

	.response-item {
		border-radius: 4px;
		cursor: pointer;
		transition: background-color 0.2s ease;
	}
	.response-item:hover {
		background-color: #f0f0f0;
	}
	.response-item + .response-item {
		margin-top: 1rem;
		border-top: 1px solid #eee;
		padding-top: 1rem;
	}

	.response-header {
		margin-bottom: 0.5rem;
		color: #666;
		font-size: 0.9em;
		display: flex;
		flex-wrap: wrap;
		gap: 0.25em 0.5em;
	}
	.response-number {
		font-weight: bold;
	}
	.response-number.clickable-backlinks {
		cursor: pointer;
	}
	.response-number.clickable-backlinks:hover {
		text-decoration: underline;
	}
	.response-author {
		font-weight: bold;
		color: #007bff;
	}

	.id-part.bannable {
		cursor: pointer;
		color: #007bff;
	}
	.id-part.bannable:hover {
		color: #0056b3;
	}

	.highlight {
		font-weight: bold;
		color: #c7254e; /* A distinct color */
		background-color: #f9f2f4;
		padding: 0 2px;
		border-radius: 2px;
	}

	.response-body {
		line-height: 1.5;
		max-height: 300px; /* 長いレスが画面を覆わないように高さを制限 */
		overflow-y: auto;
		/* 長い文字列がレイアウトを崩さないように、強制的に改行する */
		word-break: break-all;
	}

	.response-body :global(a.response-anchor) {
		color: hsl(210, 100%, 50%);
		text-decoration: none;
		font-weight: 500;
	}
	.response-body :global(a.response-anchor:hover) {
		text-decoration: underline;
	}

	.response-body :global(a.external-link) {
		color: #008000;
		text-decoration: underline;
	}
	.response-body :global(a.external-link:hover) {
		color: #006400;
	}

	.no-results {
		color: #666;
		padding: 1rem 0;
		text-align: center;
	}
</style>
