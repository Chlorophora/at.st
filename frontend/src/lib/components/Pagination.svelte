<script lang="ts">
	import { createEventDispatcher } from 'svelte';

	export let currentPage: number;
	export let totalPages: number;

	let jumpToPageInput = currentPage.toString();
	const dispatch = createEventDispatcher();

	function changePage(page: number) {
		// 入力値が有効な範囲内かチェック
		if (page >= 1 && page <= totalPages) {
			dispatch('change', page);
		}
	}

	function handleJump() {
		const page = parseInt(jumpToPageInput);
		if (!isNaN(page)) {
			changePage(page);
		}
	}

	// currentPageプロパティが親から変更されたときに、入力フィールドも更新する
	$: jumpToPageInput = currentPage.toString();
</script>

<div class="pagination">
	<button class="nav-button" on:click={() => changePage(currentPage - 1)} disabled={currentPage <= 1}> 前へ </button>
	<span class="page-info">{currentPage} / {totalPages}</span>
	<button class="nav-button" on:click={() => changePage(currentPage + 1)} disabled={currentPage >= totalPages}>
		次へ
	</button>

	<form on:submit|preventDefault={handleJump} class="jump-form">
		<input type="number" bind:value={jumpToPageInput} min="1" max={totalPages} class="jump-input" />
		<button type="submit" class="jump-button">ジャンプ</button>
	</form>
</div>

<style>
	.pagination {
		display: flex;
		justify-content: center;
		align-items: center;
		gap: 0.75rem;
		margin: 2rem 0;
	}

	.page-info {
		font-size: 0.9rem;
		color: #555;
		padding: 0 0.5rem;
		min-width: 60px; /* Prevent layout shift */
		text-align: center;
	}

	.nav-button,
	.jump-button {
		padding: 0.5rem 1rem;
		font-size: 0.9rem;
		border-radius: 6px;
		border: 1px solid #ccc;
		background-color: white;
		cursor: pointer;
		transition: background-color 0.2s, border-color 0.2s;
	}

	.nav-button:hover:not(:disabled),
	.jump-button:hover:not(:disabled) {
		background-color: #f0f0f0;
		border-color: #bbb;
	}

	.nav-button:disabled,
	.jump-button:disabled {
		background-color: #f9f9f9;
		color: #aaa;
		cursor: not-allowed;
		border-color: #e0e0e0;
	}
	.jump-form {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		margin-left: 1rem;
	}
	.jump-input {
		width: 60px;
		text-align: center;
		padding: 0.5rem;
		font-size: 0.9rem;
		border-radius: 6px;
		border: 1px solid #ccc;
	}
</style>