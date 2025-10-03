<script lang="ts">
	import type { Post, Comment } from 'src/app';
	import { createEventDispatcher } from 'svelte';
	import { fly } from 'svelte/transition';
	import { fixDoubleEscaping } from '$lib/utils/html';
	import { zIndexManager } from '$lib/stores/zIndex';
	import { clickOutside } from '$lib/actions/clickOutside';

	export let id: string;
	export let level: number;
	export let backlinks: { response: Post | Comment; number: number }[];
	export let position: { x: number; y: number };
	export let targetResponseNumber: number;
	export let backlinkCounts: Map<number, number>;
	export let asModalContent = false;

	const dispatch = createEventDispatcher();

	// コンポーネント作成時にz-indexを取得して、マウント時から最前面に表示されるようにする
	let zIndex: number | undefined = asModalContent ? undefined : zIndexManager.getNewZIndex();

	function bringToFront() {
		if (!asModalContent) {
			zIndex = zIndexManager.getNewZIndex();
		}
	}
	function handleItemClick(responseNumber: number) {
		const targetElement = document.getElementById(`res-${responseNumber}`);
		if (targetElement) {
			targetElement.scrollIntoView({ behavior: 'smooth', block: 'start' });
		}
		setTimeout(() => { // スクロールアニメーションが完了するのを待つ
			dispatch('close');
		}, 500); // 0.5秒待ってからポップアップを閉じる
	}

	function handleResponseNumberClick(e: MouseEvent | KeyboardEvent, responseNumber: number) {
		e.stopPropagation(); // .response-item のクリックイベントを抑制
		if (!backlinkCounts.has(responseNumber)) return;
		dispatch('showbacklinks', {
			responseNumber: responseNumber,
			event: e,
			level: level + 1
		});
	}

	function handleIdPartClick(
		e: MouseEvent | KeyboardEvent,
		responseId: number,
		banType: 'User' | 'Ip' | 'Device',
		idPartValue: string
	) {
		e.stopPropagation();
		dispatch('showidposts', {
			banType,
			idPartValue,
			event: e,
			sourceId: responseId
		});
	}

	function handleBodyClick(e: MouseEvent) {
		const anchor = (e.target as HTMLElement).closest('a.response-anchor');
		if (anchor) {
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

	// Helper to format the date
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
		class:backlinks-popover={!asModalContent}
		class:modal-view={asModalContent}
		style:left={!asModalContent && position ? `${position.x}px` : undefined}
		style:top={!asModalContent && position ? `${position.y}px` : undefined}
		style:z-index={zIndex}
		on:mousedown={bringToFront}
		use:clickOutside on:outclick={() => dispatch('close')}
		data-level={level}
	>
		<div class="backlinks-content">
			{#each backlinks.sort((a, b) => a.number - b.number) as { response, number } (number)}
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
						{#if backlinkCounts.get(number)}
							<span
								class="response-number clickable-backlinks"
								on:click={(e) => handleResponseNumberClick(e, number)}
								on:keydown={(e) => e.key === 'Enter' && handleResponseNumberClick(e, number)}
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
										class="id-part"
										class:bannable={response.can_moderate && type !== 'Device'}
										on:click={(e) => handleIdPartClick(e, response.id, type, part)}
										on:keydown={
											(e) => e.key === 'Enter' && handleIdPartClick(e, response.id, type, part)
										}
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
		</div>
	</div>
{/if}

<style>
	.modal-view {
		/* モーダル表示時のコンテナスタイル */
		padding: 0.5rem 0.75rem;
		/* 背景色、高さ、スクロールは親ラッパー(.mobile-modal-wrapper)が管理する */
	}

	.backlinks-popover {
		position: fixed;
		background-color: #ffffff;
		border: 1px solid #ccc;
		border-radius: 6px;
		box-shadow: 0 5px 15px rgba(0, 0, 0, 0.2);
		padding: 0.5rem 0.75rem; /* Match ResponsePopover */
		font-size: 0.9rem;
		min-width: 300px;
		max-width: 500px;
		max-height: 70vh;
		display: flex;
		flex-direction: column;
	}
	.backlinks-content {
		overflow-y: auto;
	}
	.response-item {
		/* Padding is now handled by the parent container */
		border-radius: 4px;
		cursor: pointer;
		transition: background-color 0.2s ease;
	}
	.response-item:hover {
		background-color: #f0f0f0; /* Keep hover for desktop popover */
	}
	.response-item + .response-item {
		margin-top: 1rem;
		border-top: 1px solid #eee;
		padding-top: 1rem;
	}

	.response-header {
		margin-bottom: 0.75rem; /* Match ResponsePopover */
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
	.response-body {
		line-height: 1.5;
		max-height: 300px; /* 長いレスが画面を覆わないように高さを制限 */
		overflow-y: auto;
		/* 長い文字列がレイアウトを崩さないように、強制的に改行する */
		word-break: break-all;
	}

	/* 本文内の要素のデフォルトマージンをリセットし、要素間のスペースを制御する */
	.response-body > :global(*) {
		margin: 0;
	}
	.response-body > :global(* + *) {
		margin-top: 1em;
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
</style>