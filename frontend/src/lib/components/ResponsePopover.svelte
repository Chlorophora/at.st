<script lang="ts">
	import type { Post, Comment } from 'src/app';
	import { createEventDispatcher } from 'svelte';
	import { fixDoubleEscaping } from '$lib/utils/html';
	import { zIndexManager } from '$lib/stores/zIndex';
	import { clickOutside } from '$lib/actions/clickOutside';

	export let response: Post | Comment | null = null;
	export let position: { x: number; y: number } | null = null;
	export let responseNumber: number | null = null;
	export let id: string | undefined = undefined;
	export let dataLevel: number;
	export let backlinkCount: number | undefined = undefined;
	export let asModalContent = false;

	const dispatch = createEventDispatcher();

	// コンポーネント作成時にz-indexを取得して、マウント時から最前面に表示されるようにする
	let zIndex: number | undefined = asModalContent ? undefined : zIndexManager.getNewZIndex();

	function bringToFront() {
		if (!asModalContent) {
			zIndex = zIndexManager.getNewZIndex();
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

	function handleClick() {
		if (!responseNumber) return;
		const targetId = `res-${responseNumber}`;
		const targetElement = document.getElementById(targetId);
		if (targetElement) {
			// スムーズスクロールで該当レスへ移動
			targetElement.scrollIntoView({ behavior: 'smooth', block: 'start' });
		}
		setTimeout(() => { // スクロールアニメーションが完了するのを待つ
			dispatch('close'); // 親コンポーネントにポップアップを閉じるよう通知
		}, 500); // 0.5秒待ってからポップアップを閉じる
	}

	function handleResponseNumberClick(e: MouseEvent | KeyboardEvent) {
		e.stopPropagation(); // ポップアップ全体のクリックイベントを抑制
		if (!responseNumber || !backlinkCount || backlinkCount === 0) return;
		dispatch('showbacklinks', {
			responseNumber: responseNumber,
			event: e,
			level: 0 // Backlinks popover from a ResponsePopover is always at level 0
		});
	}

	function handleIdPartClick(
		e: MouseEvent | KeyboardEvent,
		banType: 'User' | 'Ip' | 'Device',
		idPartValue: string
	) {
		e.stopPropagation();
		if (!response) return;
		dispatch('showidposts', {
			banType,
			idPartValue,
			event: e,
			sourceId: response.id
		});
	}

	function handleBodyClick(e: MouseEvent) {
		const anchor = (e.target as HTMLElement).closest('a.response-anchor');
		if (anchor) {
			// e.stopImmediatePropagation(); // ポップアップ全体のクリックイベント(handleClick)を抑制
			e.preventDefault(); // デフォルトのアンカー挙動（ページ内ジャンプ）を抑制
			const href = anchor.getAttribute('href');
			const match = href?.match(/#res-(\d+)/);
			if (match?.[1]) {
				dispatch('showresponse', {
					responseNumber: parseInt(match[1], 10), 
					event: e,
					level: dataLevel
				});
			}
		}
	}

	function linkifyUrls(text: string): string {
		if (!text) return '';
		const urlRegex = /(https?:\/\/[^\s<>"']+)/g;
		return text.replace(urlRegex, (url) => {
			return `<a href="${url}" target="_blank" rel="noopener noreferrer" class="external-link">${url}</a>`;
		});
	}
	$: levelDisplay = getLevelDisplay(response);
</script>

{#if response && responseNumber && (asModalContent || position)}
	<article
		id={id}
		class:popover={!asModalContent}
		class:modal-view={asModalContent}
		style:left={!asModalContent && position ? `${position.x}px` : undefined}
		style:top={!asModalContent && position ? `${position.y}px` : undefined}
		style:z-index={zIndex}
		on:mousedown={bringToFront}
		use:clickOutside on:outclick={() => dispatch('close')}
		on:mouseenter
		on:mouseleave
		data-tree-id={$$props['data-tree-id']}
		data-level={dataLevel}
	>
		<div
			class="response-item"
			on:click={(e) => {
				// レスアンカーのクリックでない場合のみポップアップ全体のクリックとして扱う
				if (!(e.target as HTMLElement).closest('a.response-anchor')) {
					handleClick();
				}
			}}
			on:keydown={(e) => e.key === 'Enter' && handleClick()}
			role="button"
			tabindex="0"
		>
			<header class="response-header">
			{#if backlinkCount && backlinkCount > 0}
				<span
					class="response-number clickable-backlinks"
					on:click={handleResponseNumberClick}
					on:keydown={(e) => e.key === 'Enter' && handleResponseNumberClick(e)}
					role="button"
					tabindex="0" 
				>
					{responseNumber}: ({backlinkCount})
				</span>
			{:else}
				<span class="response-number">{responseNumber}:</span>
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
							on:click={(e) => handleIdPartClick(e, type, part)}
							on:keydown={
								(e) => e.key === 'Enter' && handleIdPartClick(e, type, part)
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
		</div>
	</article>
{/if}

<style>
	.modal-view {
		/* モーダル表示時のコンテナスタイル */
		padding: 0.5rem 0.75rem;
		/* 背景色、高さ、スクロールは親ラッパー(.mobile-modal-wrapper)が管理する */
	}

	.popover {
		position: fixed;
		background-color: #ffffff;
		border: 1px solid #ccc;
		border-radius: 6px;
		box-shadow: 0 5px 15px rgba(0, 0, 0, 0.2);
		padding: 0.5rem 0.75rem; /* 余白を削減 */
		max-width: 500px;
		min-width: 300px;
		pointer-events: auto;
		font-size: 0.9rem;
	}

	.response-item {
		border-radius: 4px;
		cursor: pointer;
		transition: background-color 0.2s ease;
	}
	.response-item:hover {
		background-color: #f0f0f0;
	}

	.response-header {
		margin-bottom: 0.75rem;
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
	}
	.response-author {
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
		color: hsl(210 100% 50%);
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