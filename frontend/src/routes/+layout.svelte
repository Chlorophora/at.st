<script lang="ts">
	import { page } from '$app/stores';
	import type { LayoutData } from './$types';
	import { goto } from '$app/navigation';
	import '../app.css'; // グローバルなCSSをインポート
	import { onMount, tick } from 'svelte';
	import { openCommentFormPanel, openThreadFormPanel } from '$lib/stores/ui';
	import { currentThreadResponseCount } from '$lib/stores/thread';
	import { decodeHtmlEntities } from '$lib/utils/html';

	export let data: LayoutData;

	let headerElement: HTMLElement;
	let copyButtonText = '専ブラURL';

	$: dedicatedBrowserUrl = (() => {
		if (!$page.data.board || !$page.data.post) return null;
		const timestamp = Math.floor(new Date($page.data.post.created_at).getTime() / 1000);
		return `/boards/test/read.cgi/${$page.data.board.id}/${timestamp}`;
	})();

	async function handleCopyUrl() {
		if (!dedicatedBrowserUrl) return;
		try {
			await navigator.clipboard.writeText(location.origin + dedicatedBrowserUrl);
			copyButtonText = 'コピーしました';
			setTimeout(() => (copyButtonText = '専ブラURL'), 2000);
		} catch (err) {
			console.error('クリップボードへのコピーに失敗しました:', err);
		}
	}

	// onMountを使用して、コンポーネントがDOMにマウントされた後に実行します。
	onMount(() => {
		// ヘッダーの高さを計算し、CSSカスタムプロパティとして設定する関数
		const updateHeaderHeight = async () => {
			// DOMの更新が完了するのを待ってから高さを計算
			await tick();
			if (headerElement) {
				const height = headerElement.clientHeight;
				document.documentElement.style.setProperty('--header-height', `${height}px`);
			}
		};

		// 初期表示時とウィンドウリサイズ時に高さを設定
		updateHeaderHeight();
		window.addEventListener('resize', updateHeaderHeight);

		// $page ストアを購読し、ページデータ（特に$page.data.post）が変更されたときに
		// ヘッダーの高さが再計算されるようにします。
		const unsubscribePage = page.subscribe(updateHeaderHeight);

		// コンポーネントが破棄されるときに、イベントリスナーとストアの購読を解除します。
		return () => {
			window.removeEventListener('resize', updateHeaderHeight);
			unsubscribePage();
		};
	});
</script>

<div class="app-container">
	<header class="app-header" bind:this={headerElement}>
		<nav class="main-nav">
			<div class="nav-left">
				<a href="/">☕</a>
				{#if $page.data.board}
					<span class="breadcrumb-separator">&gt;</span>
					<a href="/boards/{$page.data.board.id}" class="board-context">
						{$page.data.board.name} (#{$page.data.board.id})
					</a>
				{/if}
			</div>
			<div class="user-actions">
				<!-- トップページでのみ「新しい板を作成する」ボタンを表示 -->
				{#if $page.url.pathname === '/'}
					<button class="create-board-btn-header" on:click={() => goto(data.user ? '/boards/create' : '/auth/register')}>
						新しい板を作成する
					</button>
				{/if}

				<!-- 専ブラURLボタン (スレッドページでのみ表示) -->
				{#if dedicatedBrowserUrl}
					<button class="header-action-btn" on:click={handleCopyUrl}
						>{copyButtonText}</button
					>
				{/if}

				{#if data.user}
					<!-- マイページ (ログイン時) -->
					<a href="/mypage">マイページ</a>

					{#if data.user?.role?.toLowerCase() === 'admin'}
						<a href="/admin/bans">adminBAN管理</a>
						<a href="/admin/users">ユーザー</a>
						<a href="/admin/failed-verifications">認証失敗ログ</a>
						<a href="/admin/settings">各種設定</a>
						<a href="/admin/rate-limits">レート制限</a>
						<a href="/id-search">必死チェッカー</a>
					    <a href="/archive">過去ログ検索</a>
					{/if}
				{:else}
					<a href="/auth/register">認証</a>
				{/if}
			</div>
		</nav>
		<!-- 2段目のヘッダー: スレッドページでのみ表示 -->
		{#if $page.data.post}
			<div class="sub-header">
				<div class="sub-header-content">
					<span class="breadcrumb-separator">&gt;</span>
					<h1 class="thread-title-header">
						<a href="/posts/{$page.data.post.id}" title={decodeHtmlEntities($page.data.post.title)}>
							{@html decodeHtmlEntities($page.data.post.title)}
						</a>
						{#if $currentThreadResponseCount !== null}
							<span class="comment-count">({$currentThreadResponseCount})</span>
						{/if}
					</h1>
				</div>
			</div>
		{/if}
	</header>
	<main>
		<slot />

	</main>
</div>

<style>
	:global(body) {
		font-family: sans-serif;
		background-color: #f0f2f5;
		color: #333;
		margin: 0;
	}

	.app-container {
		display: flex;
		flex-direction: column;
		min-height: 100vh;
	}

	.app-header {
		position: sticky;
		top: 0;
		z-index: 1000;
		background-color: #343a40;
		box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
	}

	.main-nav {
		/* background-color は .app-header に移動しました */
		padding: 0.25rem 1.5rem; /* ヘッダー全体の余白を削減 */
		/* border-radius と margin-bottom はスティッキーヘッダーには不要です */
		display: flex;
		justify-content: space-between;
		align-items: center;
		/* ナビゲーションコンテンツを中央に配置します */
		max-width: 1940px;
		margin: 0 auto;
	}

	.nav-left {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.breadcrumb-separator {
		color: #adb5bd;
	}

	.main-nav a {
		color: white;
		text-decoration: none;
		font-weight: bold;
	}

	.user-actions {
		display: flex;
		align-items: center;
		gap: 1rem;
	}

	/* ボタンの有無でヘッダーの高さが変わらないように、通常のリンクにも縦の余白を確保 */
	.user-actions > a:not(.create-board-btn-header) {
		padding-top: 0.4rem;
		padding-bottom: 0.4rem;
	}

	.create-board-btn-header {
		padding: 0.4rem 0.4rem; /* ボタンの縦の余白を調整 */
		background-color: #007bff;
		color: white;
		border: none;
		border-radius: 4px;
		cursor: pointer;
		font-size: 0.9em;
		font-weight: bold;
		transition: background-color 0.2s ease;
		white-space: nowrap;
	}

	.create-board-btn-header:hover {
		background-color: #0056b3;
	}

	main {
		flex-grow: 1;
		max-width: 1940px; /* 960px * 1.5, 管理者ページ等のために幅を広げる */
		width: 100%;
		margin: 1.5rem auto;
		padding: 1rem;
		box-sizing: border-box;
	}

	.sub-header {
		background-color: #495057; /* メインヘッダーより少し明るい色 */
		padding: 0.5rem 1.5rem; /* 上下のパディングを少し広げる */
		box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.1);
		box-sizing: border-box;
		display: flex;
		align-items: center; /* コンテンツを垂直方向に中央揃えする */
		/* タイトルの長さに応じて高さが可変になるように、固定の高さを削除 */
	}

	.sub-header-content {
		/* 中央揃え(margin: 0 auto)と最大幅(max-width)を削除し、左寄せにします */
		width: 100%;
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.comment-count {
		margin-left: 0.5ch;
		font-size: 1em; /* 親要素(h1)のフォントサイズに相対的にする */
		color: #f8f9fa;
		white-space: nowrap; /* レス数が改行されないようにする */
	}

	.thread-title-header {
		/* フォントサイズは固定 */
		font-size: 1rem;
		line-height: 1.4;
		margin: 0;
		color: #f8f9fa;
		/* 全文表示を優先するため、テキストの折り返しを許可します */
		white-space: normal;
		word-break: break-word;
	}

	.header-action-btn {
		background: none;
		border: none;
		/* 他のリンクと高さを合わせる */
		padding-top: 0.4rem;
		padding-bottom: 0.4rem;
		color: white;
		text-decoration: none;
		font-weight: bold;
		font-family: inherit;
		font-size: 1em;
		cursor: pointer;
		transition: opacity 0.2s ease;
	}
	.header-action-btn:hover {
		opacity: 0.8;
	}
	.thread-title-header a {
		color: inherit;
		text-decoration: none;
	}

	/* --- スマートフォン向けのスタイル --- */
	@media (max-width: 768px) {
		:global(html) {
			/* モバイルでは基準フォントサイズを小さくし、rem単位の要素も追従させる */
			font-size: 14px;
		}
		:global(body) {
			/* 可読性のために行間を確保 */
			line-height: 1.5;
		}

		.main-nav {
			/* パディングを少し詰める */
			padding: 0.25rem 1rem;
		}

		.user-actions {
			/* リンク間のギャップを詰める */
			gap: 0.5rem;
		}

		main {
			padding: 0 0.75rem; /* 左右のパディングのみ設定 */
			margin: 1rem auto; /* 上下のマージンを少し詰める */
		}

		.sub-header {
			/* サブヘッダーの左右の余白を詰める */
			padding: 0.3rem 0.3rem;
		}

		.thread-title-header {
			/* スレッドタイトルのフォントサイズを小さくする */
			/* clamp(最小値, 推奨値, 最大値) を使用して、画面幅に応じてフォントサイズを動的に変更 */
			font-size: clamp(0.8rem, 2.5vw, 0.9rem);
		}
	}
</style>